#include <emscripten/html5.h>

int pergel_gl_create(const char *selector) {
  EmscriptenWebGLContextAttributes attrs;
  emscripten_webgl_init_context_attributes(&attrs);
  attrs.majorVersion = 2;
  attrs.minorVersion = 0;
  attrs.alpha = 0;
  attrs.depth = 0;
  attrs.stencil = 1;
  attrs.antialias = 0;
  attrs.premultipliedAlpha = 1;
  attrs.preserveDrawingBuffer = 0;
  attrs.powerPreference = EM_WEBGL_POWER_PREFERENCE_HIGH_PERFORMANCE;
  attrs.enableExtensionsByDefault = 1;

  EMSCRIPTEN_WEBGL_CONTEXT_HANDLE ctx = emscripten_webgl_create_context(selector, &attrs);
  if (ctx <= 0) {
    return (int)ctx;
  }
  emscripten_webgl_make_context_current(ctx);
  return (int)ctx;
}
