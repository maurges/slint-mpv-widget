# Slint mpv widget

Not a complete widget, but it's an example of drawing mpv video into slint texture.

This will create a window with a video playing and some controls. To build, you
can use nix or you can link to libmpv yourself.

## Some notes on implementation

1. I haven't completely figured out how texture+image are supposed to work.
   Here I don't recreate neither unless I need to change size, and drawing to
   old texture will update the image, which is expected opengl behaviour. But
   in examples textures are recreated on every frame, sooo who's wrong?

2. Audio events are fucked

3. When setting an mpv property from rust, the property update event is not
   emitted from mpv. This is logical, but makes some things inconvenient, as
   you have to set slint property in several places.

4. Didn't bother with well-typed mpv commands, but would like if someone
   implemented them. For inspiration, you can look at mpv qt example.

5. Performance is great even in debug mode, unsurprisingly.

## Thoughts on slint, again

Again I find that slint is more restrictive than I hoped after QML. I'm still
baffled there are no general `on_property_change` events: makes bidirectional
bindings almost useless, as you can't conveniently update your data model from
rust.

It would be better if slint opengl also created a context for you with a
framebuffer shared, like in qt. It took me a long time to figure out how to
make it so mpv wouldn't fuck up slint's drawing context, and I'm still not
sure why it works here but not in my similar c++ project. Maybe because it
already does that?

I would like to define a component type with properties and callbacks, use this
component as a global, and then access this global from rust and from slint
both - to set mpv properties on one end and to read them on the other. This is
the only design I saw for making this into a proper widget without having to
write too much boilerplate for each instance. But obvously there is a problem
that globals in rust are selected by type, so this makes defining two globals
impossible. Alternatively, it would be nice to define a global with a component
as a property, which is also impossible since components aren't first class -
another big limitation after QML.

In general, it would be nice to have a slint widget backed by rust, drawing and
updating included, similar to QQuickItem/QQuickFramebufferObject. I know, this
breaks the slint independence from the backend code, but come on. I believe
there has to be a way to have both.

Unrelated, but I just discovered that slint doesn't have a StackView analogue?
Wtf, speak about backend independence. Because the forum replies recommend
setting up an if-else widget chain dispatched in a stack-like manner from rust,
which is completely non-transparent.

Rich text when?

Anyways, thanks for reading this till the end, madnirua and/or tronical and/or
other slint creators. Even with my complaining, it's still an awesome library
and is the closest to something I imagined as a perfect GUI.
