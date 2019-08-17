# Diffusion Limited Aggregation system

An implementation of [Diffusion Limited Aggregation systems][dla] that can be
rendered by using povray or can be explored interactively by using a dead simple
viewer.

## Povray

It's possible to save the DLA system as a pov file ready to be rendered with
povray. Here's an example to render 10 millions particles on a 4K canvas.

```shell
$ cargo run --release -- -p 10000000 -a 8 -g 30 -s povray
$ povray +A +W4096 +H4096 dla.pov
```

## Interactive JS viewer

It's also possible to dump the state of the DLA system as a plain JS file that
can be used to power great web visualizations. An example viewer is provided and
here's an example on how to explore a system made of 10K particles.

```shell
$ cargo run --release -- -p 10000 -a 8 -g 30 -s js
$ firefox index.html
```

## Raw CSV dump

In case you want to render the system by yourself then feel free to use the
`csv` scene format to save the cells in the DLA system. As an example, take a
look at how I can render it with [Buzz, my ray tracer][particles-buzz].

```shell
$ cargo run --release -- -p 10000 -a 8 -g 30 -s csv
$ cd ../r3d && cargo run --release --example particles < dla/dla.csv
```

## Example

![dla-2k](images/dla-small.png)
![dla-2k-buzz](images/dla-small-buzz.png)

[dla]: https://en.wikipedia.org/wiki/Diffusion-limited_aggregation
[particles-buzz]: https://github.com/d-dorazio/r3d/blob/master/buzz/examples/particles.rs
