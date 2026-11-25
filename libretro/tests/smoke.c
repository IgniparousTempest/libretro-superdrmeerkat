/* Exercise the packaged core against the frontend ABI on OpenGL ES 2. */
#include <assert.h>
#include <stdint.h>
#include <string.h>
#include <EGL/egl.h>
#include <GLES2/gl2.h>
#include <libretro-common/libretro.h>

static struct retro_hw_render_callback hw;
static GLuint fbo;
static unsigned width, height, frames, polls, audio_frames;
static uint16_t buttons;
static bool no_game, audible;

static uintptr_t framebuffer(void) { return fbo; }

static bool environment(unsigned cmd, void *data) {
    switch (cmd) {
        case RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME:
            no_game = *(const bool *)data;
            return true;
        case RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER:
            *(enum retro_hw_context_type *)data = RETRO_HW_CONTEXT_OPENGLES2;
            return false;
        case RETRO_ENVIRONMENT_SET_HW_RENDER: {
            struct retro_hw_render_callback *request = data;
            assert(request->context_type == RETRO_HW_CONTEXT_OPENGLES2);
            request->get_current_framebuffer = framebuffer;
            request->get_proc_address = eglGetProcAddress;
            hw = *request;
            return true;
        }
        default: return false;
    }
}

static void poll(void) { ++polls; }
static int16_t input(unsigned port, unsigned device, unsigned index, unsigned id) {
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
        glReadPixels(0, h * 2 / 3, 640, 1, GL_RGBA, GL_UNSIGNED_BYTE, row);
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

int main(int argc, char **argv) {
    assert(argc == 2 && retro_api_version() == RETRO_API_VERSION);
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
    assert(no_game && retro_load_game(&(struct retro_game_info){.path = argv[1]}));
    struct retro_system_av_info av;
    retro_get_system_av_info(&av);
    width = av.geometry.base_width;
    height = av.geometry.base_height;
    assert(width == 640 && height == 360 && av.timing.fps == 60 && av.timing.sample_rate == 44100);
    glGenFramebuffers(1, &fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, fbo);
    GLuint texture;
    glGenTextures(1, &texture);
    glBindTexture(GL_TEXTURE_2D, texture);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, texture, 0);
    assert(glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE);
    hw.context_reset();
    buttons = 1 << RETRO_DEVICE_ID_JOYPAD_SELECT;
    retro_run();
    assert(audible);
    buttons = 1 << RETRO_DEVICE_ID_JOYPAD_START;
    retro_run();
    buttons = 1 << RETRO_DEVICE_ID_JOYPAD_R;
    retro_run();
    assert(!audible);
    hw.context_destroy();
    hw.context_reset();
    retro_reset();
    buttons = 0;
    retro_run();
    assert(frames == 4 && polls == 4 && audio_frames == 4 * 735);
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
