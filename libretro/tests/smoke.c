/* Exercise the packaged core against the frontend ABI on OpenGL ES 2. */
#include <assert.h>
#include <stdint.h>
#include <string.h>
#include <EGL/egl.h>
#include <GLES2/gl2.h>
#include <libretro-common/libretro.h>

static struct retro_hw_render_callback hw;
static GLuint fbo, texture;
static unsigned width, height, frames, polls, audio_frames, polled_ports;
static uint16_t buttons;
static bool no_game, audible;
static int max_users = -1;
static const char *resolution;
static bool option_registered, option_changed, reject_resize;
#ifdef RETRO_ENVIRONMENT_GET_DISPLAY_METADATA
static bool report_display;
#endif

static uintptr_t framebuffer(void) { return fbo; }

static bool environment(unsigned cmd, void *data) {
    switch (cmd) {
        case RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME:
            no_game = *(const bool *)data;
            return true;
        case RETRO_ENVIRONMENT_GET_INPUT_MAX_USERS:
            if (max_users < 0) return false;
            *(unsigned *)data = (unsigned)max_users;
            return true;
        case RETRO_ENVIRONMENT_SET_VARIABLES: {
            const struct retro_variable *opts = data;
            assert(strcmp(opts[0].key, "superdrmeerkat_resolution") == 0);
            assert(strstr(opts[0].value, "; Auto|") && opts[1].key == NULL);
            option_registered = true;
            return true;
        }
        case RETRO_ENVIRONMENT_GET_VARIABLE: {
            struct retro_variable *var = data;
            assert(strcmp(var->key, "superdrmeerkat_resolution") == 0);
            var->value = resolution;
            return resolution != NULL;
        }
        case RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE:
            *(bool *)data = option_changed;
            option_changed = false;
            return true;
        case RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO: {
            const struct retro_game_geometry *geo = &((const struct retro_system_av_info *)data)->geometry;
            if (reject_resize) return false;
            hw.context_destroy();
            glBindTexture(GL_TEXTURE_2D, texture);
            glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, geo->max_width, geo->max_height,
                         0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
            hw.context_reset();
            width = geo->base_width;
            height = geo->base_height;
            return true;
        }
        case RETRO_ENVIRONMENT_SET_GEOMETRY: {
            const struct retro_game_geometry *geo = data;
            width = geo->base_width;
            height = geo->base_height;
            return true;
        }
#ifdef RETRO_ENVIRONMENT_GET_DISPLAY_METADATA
        case RETRO_ENVIRONMENT_GET_DISPLAY_METADATA: {
            if (!report_display) return false;
            struct retro_display_metadata *display = data;
            assert(display->version == 1 && display->size >= sizeof(*display));
            display->geo.native_width = 1024;
            display->geo.native_height = 768;
            return true;
        }
#endif
        case RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER:
            /* RetroArch's Pi "gl" driver may expose only GLES without driver switching. */
            *(enum retro_hw_context_type *)data = RETRO_HW_CONTEXT_OPENGL;
            return false;
        case RETRO_ENVIRONMENT_SET_HW_RENDER: {
            struct retro_hw_render_callback *request = data;
            if (request->context_type != RETRO_HW_CONTEXT_OPENGLES2) return false;
            request->get_current_framebuffer = framebuffer;
            request->get_proc_address = eglGetProcAddress;
            hw = *request;
            return true;
        }
        default: return false;
    }
}

static void poll(void) {
    ++polls;
    polled_ports = 0;
}
static int16_t input(unsigned port, unsigned device, unsigned index, unsigned id) {
    assert(port < 4);
    polled_ports |= 1u << port;
    return port == 1 && device == RETRO_DEVICE_JOYPAD && index == 0 && (buttons & (1u << id));
}
static size_t audio(const int16_t *samples, size_t count) {
    assert(count == 735);
    audible = false;
    for (size_t i = 0; i < count; ++i) {
        assert(samples[2*i] == samples[2*i + 1]);
        audible |= samples[2*i] != 0;
    }
    audio_frames += count;
    return count;
}
static void video(const void *data, unsigned w, unsigned h, size_t pitch) {
    assert(data == RETRO_HW_FRAME_BUFFER_VALID && w == width && h == height && pitch == 0);
    GLint bound;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &bound);
    assert((GLuint)bound == fbo);
    if (frames == 0) {
        /* The title row crosses its dark background and the coloured wordmark. */
        uint8_t row[640 * 4];
        glReadPixels((w - 640) / 2, h * 2 / 3, 640, 1, GL_RGBA, GL_UNSIGNED_BYTE, row);
        unsigned changes = 0;
        for (unsigned i = 1; i < 640; ++i) changes += memcmp(row, row + i*4, 3) != 0;
        assert(changes > 40);
    }
    assert(glGetError() == GL_NO_ERROR);
    ++frames;
    /* The frontend owns GL state between retro_run calls. */
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    glEnable(GL_SCISSOR_TEST);
    glScissor(0, 0, 0, 0);
    glColorMask(GL_FALSE, GL_FALSE, GL_FALSE, GL_FALSE);
    glBindBuffer(GL_ARRAY_BUFFER, 0);
    glBindTexture(GL_TEXTURE_2D, 0);
    glUseProgram(0);
}

static void check_load_size(unsigned w, unsigned h) {
    struct retro_system_av_info av;
    assert(retro_load_game(NULL));
    retro_get_system_av_info(&av);
    assert(av.geometry.base_width == w && av.geometry.base_height == h);
    retro_unload_game();
}

int main(void) {
    assert(retro_api_version() == RETRO_API_VERSION);
    struct retro_system_info info;
    retro_get_system_info(&info);
    assert(info.valid_extensions && !*info.valid_extensions && !info.need_fullpath);
    EGLDisplay display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    assert(eglInitialize(display, NULL, NULL));
    EGLint count;
    EGLConfig config;
    const EGLint attrs[] = {EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
                            EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_NONE};
    assert(eglChooseConfig(display, attrs, &config, 1, &count) && count == 1);
    const EGLint version[] = {EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE};
    EGLContext context = eglCreateContext(display, config, EGL_NO_CONTEXT, version);
    EGLSurface surface = eglCreatePbufferSurface(display, config,
        (EGLint[]){EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE});
    assert(eglMakeCurrent(display, surface, surface, context));

    retro_set_environment(environment);
    retro_set_video_refresh(video);
    retro_set_audio_sample_batch(audio);
    retro_set_input_poll(poll);
    retro_set_input_state(input);
    retro_init();
    assert(no_game && option_registered);
    check_load_size(1920, 1080);
    resolution = "Auto";
#ifdef RETRO_ENVIRONMENT_GET_DISPLAY_METADATA
    report_display = true;
    check_load_size(1024, 768);
#else
    check_load_size(1920, 1080);
#endif
    resolution = "960x540";
    assert(retro_load_game(NULL));
    struct retro_system_av_info av;
    retro_get_system_av_info(&av);
    width = av.geometry.base_width;
    height = av.geometry.base_height;
    assert(width == 960 && height == 540 && av.timing.fps == 60 && av.timing.sample_rate == 44100);
    glGenFramebuffers(1, &fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, fbo);
    glGenTextures(1, &texture);
    glBindTexture(GL_TEXTURE_2D, texture);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, texture, 0);
    assert(glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE);
    hw.context_reset();
    max_users = 2;
    buttons = 1 << RETRO_DEVICE_ID_JOYPAD_SELECT;
    retro_run();
    assert(audible && polled_ports == 0x3);
    retro_set_controller_port_device(0, RETRO_DEVICE_NONE);
    buttons = 1 << RETRO_DEVICE_ID_JOYPAD_START;
    retro_run();
    assert(polled_ports == 0x2);
    max_users = 8;
    buttons = 1 << RETRO_DEVICE_ID_JOYPAD_R;
    retro_run();
    assert(!audible && polled_ports == 0xe);
    hw.context_destroy();
    hw.context_reset();
    retro_reset();
    max_users = -1;
    retro_set_controller_port_device(0, RETRO_DEVICE_JOYPAD);
    buttons = 0;
    retro_run();
    assert(polled_ports == 0xf);

    max_users = 0;
    resolution = "1280x720";
    option_changed = true;
    reject_resize = true;
    retro_run();
    assert(polled_ports == 0 && width == 960 && height == 540);
    max_users = 1;
    option_changed = true;
    reject_resize = false;
    retro_run();
    assert(polled_ports == 1 && width == 1280 && height == 720);
    max_users = 2;
    retro_set_controller_port_device(0, RETRO_DEVICE_NONE);
    retro_set_controller_port_device(1, RETRO_DEVICE_JOYPAD);
    retro_set_controller_port_device(3, RETRO_DEVICE_ANALOG);
    resolution = "960x540";
    option_changed = true;
    retro_run();
    retro_get_system_av_info(&av);
    assert(polled_ports == 0xa);
    assert(width == 960 && height == 540 && av.geometry.max_width == 1280 && av.geometry.max_height == 720);
    assert(frames == 7 && polls == 7 && audio_frames == 7 * 735);
    hw.context_destroy();
    retro_unload_game();
    retro_deinit();
    glDeleteTextures(1, &texture);
    glDeleteFramebuffers(1, &fbo);
    eglMakeCurrent(display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    eglDestroyContext(display, context);
    eglDestroySurface(display, surface);
    eglTerminate(display);
    return 0;
}
