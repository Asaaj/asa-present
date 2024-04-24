# Overview

This is an early-stage project intended to make programming presentations more interactive. It may be used with a 
Javascript-based presentation framework (e.g. [reveal.js](https://revealjs.com/)) to provide the following features:

1. Preload code snippets in the slide.
2. Edit code live (using [Monaco](https://microsoft.github.io/monaco-editor/)).
3. Compile and run code examples without leaving the slide deck.

Importantly, the code examples will run in WebAssembly, meaning it can access DOM elements _inside the slide deck_,
like `<canvas>` for rendering, or `<div>`s for "printing" text. This offers a great deal of flexibility for live
interactivity, without the need to switch desktops, tabs, or windows to show the live code example (which never seems to
go smoothly when under the pressure of a live talk).

# Use Cases

## WASM + WebGL Presentation

Say, for example, I would like to present about using WebGL from WebAssembly. I could simply show code snippets and 
screenshots, which I have personally done in the past. But this is not very engaging.

I can go one step further by having a compiled WASM project that gets loaded into the slideshow. This is somewhat better
because it allows me to demonstrate a real project, such as a rendering pipeline, right inside the slides. I gave a 
[lightning talk](https://youtu.be/LmH7b5OI4VY?si=ShSAQh49LJXyl0WX) using this method. However, it has a few serious
drawbacks:

1. The slide code can easily get out of date, since it has to be manually copied from the project into the slide.
2. The compiled WASM loaded into the slide can easily get out of date, since it has to be manually built before starting
the presentation.
3. There is no way to change the code once the presentation is started, making audience questions entirely theoretical.

That's where this framework comes in. Not only could I implement the demo project at the same time as editing my slides,
but I can also update code during the presentation, and hook the compile/run step to automatically keep things up to 
date when I start presenting.

## Educational Videos

Many educational video creators use slides as the core of their formats. This makes a lot of sense because it's a
familiar medium to many educators, and it cuts out a lot of the editing overhead required by more advanced presentation
media. 

However, these slides suffer from the same issues in pre-recorded videos as in live presentations. Video platforms are
still less interactive than live presentations, so some of the issues are even worse. This framework can offer a few
key benefits:

1. The resulting slides can be provided as a downloadable repo for viewers to play around with.
2. The resulting video integrates the edit/run loop without additional video editing overhead or window management.

# Goals

Aside from the obvious compile/run-in-browser loop, the biggest feature I want to add is intra-slide composability. The
idea is to be able to build upon previous slides, which is a common practice in presentations. A module written in a
previous slide should be able to be imported and used by a subsequent slide. 

The framework also needs to support offline mode, since live presentations should not rely on an internet connection
(conference WiFi is generally pretty bad).

I'm currently planning to support Rust and C++ for WASM. Of course, this comes with Javascript support to run functions
from the WASM, and I think Python would be pretty simple to add.

# Implementation

The core compiling infrastructure in this repository is only a sightly modified copy of the 
[`rust-playground`](https://github.com/rust-lang/rust-playground). It is not designed for scale right now, as it is 
initially intended to only have one active client: the on-machine presentation.

Eventually, designing for scale would be very handy, as it would allow for educators to simply publish slides as a 
website, and users could modify and run the code right in the slides. For now, though, the users will have to build
and run the server themselves.

# Alternatives

You may be wondering, "why not just embed a self-hosted instance of Compiler Explorer in the slides?" While that is an
option, it does not provide the same flexibility for running interactive examples, such as rendering to a `<canvas>` or
importing previously-written code from other slides.

Ideally, one would be able to run the compiler directly inside the browser. That appears to be impossible right now for
[Rust](https://github.com/rust-lang/miri/issues/722), so since that's the main language I'm targeting, I haven't done 
any research into doing it for C++. If I have to run the server anyway, I might as well use it for all of the languages.
