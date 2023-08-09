
# tabbed-rs

`tabbed-rs` is a rust rewrite/clone of [tabbed](https://tools.suckless.org/tabbed/). In brief, `tabbed` has bugs which annoy me and lacks certain specific behaviors which I hacked together with shell scripts.

This repo also containes a `tabctrl` binary for manipulating `tabbed-rs` windows, which is inspired by [bsptab](https://github.com/albertored11/bsptab). For now it's only really usable in bspwm but will be made more general later.

![image](https://github.com/yrkv/tabbed-rs/assets/11140316/aa2d6e29-f345-4a6f-a945-42b1ac893295)

Note the tabs at the top of the screenshot, much like in `tabbed`.

## Why?

`tabbed` has a few bugs that were annoying me and I'm not a huge fan of the extremely minimal appearance. I was able to deal with everything with patches and a collection of shell scripts, but it was a mess. Since `tabbed` is small and minimal enough, it's not too bad to remake it completely with my use case in mind.

## What?

`tabbed-rs` is more or less equivalent to `tabbed`, though with a different appearance and some slightly different behaviors. 

`tabctrl` is a convenience utility for manipulating tabbed windows. It currently is specific to only bspwm by relying on the same mechanisms used by `bsptab`, but this will change eventually.

### TODO

- [WIP] Configuration files. Not sure what the best way to handle it is
- Replicate more features of `tabbed`
- Document the details of `tabbed-rs` and `tabctrl` more
- Remove WM-specific things in `tabctrl`
- Make `tabctrl` also work with `tabbed`
