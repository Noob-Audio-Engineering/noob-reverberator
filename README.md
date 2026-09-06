# Noob Reverberator

> **About Noob Reverberator.** I wrote it as a humorous, affectionate answer to
> the reverbs I admire — Valhalla's, FabFilter's Pro-R, Strymon's BigSky. It is
> a free plug-in from Noob Audio Engineering, built to show what
> [noob-vst-webgui-framework](https://github.com/Noob-Audio-Engineering/noob-vst-webgui-framework)
> can do with a product-sized interface. It is my tribute to work I admire, not
> a parity replacement for any of it — and **nothing here claims to beat any of
> them**, because I have measured none of them. See
> [What this does not claim](#what-this-does-not-claim).

A free algorithmic reverb by Noob Audio Engineering. The plug-in window is the
operating system's web view; what it shows is a Vue single-page app served by
the plug-in itself and driven over the framework's local WebSocket bridge. The
DSP, the parameters and the page are all in this crate; everything reusable is
in the framework.

**You draw the decay time against frequency, and the network is fitted to it.**
Not a high cut and a bass multiplier — a curve you make by clicking on it, the
way Noob-Q's is made, and the panel draws what the running filters actually
produce beside what you asked for.

Click the curve to put a band where you want it; the shape it starts as depends
on where you clicked, because somebody reaching for the bottom or the top of
the spectrum almost always wants a shelf. Drag it to shape it, double-click or
right-click to take it away. Each band has its own colour, spaced by the golden
angle so that neighbours — the pairs anybody actually compares — stay far apart
in hue however many are on screen. **Up to thirty-two.**

Under the curve, the tail's own spectrum, in its own strip with its own scale.
Not scaled onto the decay axis, which would be a lie: that axis is seconds and
this is decibels. It shares the frequency axis, which is what makes it readable
against the curve, and says nothing about the other one.

Controls the running architecture never reads are dimmed and unreachable, with
the reason in the panel header — the spring's tension on a plate, the decay
curve on the early-reflections-only mode. Controls that are merely *at zero*
are not dimmed: Bloom on Concert Hall works the moment it is turned up.

## Install it

Every commit on `main` is built for Windows and macOS and published to the
rolling [`latest`](https://github.com/Noob-Audio-Engineering/noob-reverberator/releases/tag/latest)
release: a VST3 and a CLAP in one zip, with a checksummed manifest beside them
and a photograph of that build running.

The easy way is the [Noob Plugin
Manager](https://github.com/Noob-Audio-Engineering/noob-plugin-manager), which
installs and updates every plug-in in this organisation and verifies the
checksum before it writes anything.

## What it is made of

| Part | Where | Role |
|---|---|---|
| DSP | `src/dsp/` | The decay curve, the loss fitting, four architectures, and the modifiers. Host-agnostic. |
| Plug-in | `src/plugin.rs` (feature `plugin`) | nih-plug VST3 / CLAP effect. Embeds `web/dist`. |
| Standalone | `src/bin/standalone.rs` | Fake audio thread on a demo source plus the framework's server: interface development without a DAW. |
| Measurement | `src/bin/benchmark.rs` | Writes [`docs/BENCHMARK.md`](docs/BENCHMARK.md). |
| SPA | `web/` | The interface. |

## Build and run

```sh
# 1. The page (once, and after every interface change)
cd web && npm install && npm run build && cd ..

# 2. The standalone, to see it without a DAW
cargo run --release --bin noob-reverberator-standalone -- --open

# 3. The plug-in itself. `--features plugin` is not optional: without it
#    cargo builds the library with no entry points and the bundle is a shell.
cargo nih-plug bundle noob-reverberator --release --features plugin
```

## The one decision this plug-in is about

A delay line's loss per trip is fixed by how long the tail should last:

```text
loss per trip = −60 · (m / fs) / T60   decibels
```

so if you say what `T60` should be **at every frequency**, the filter each line
needs is determined. That is the whole idea, and FabFilter's Pro-R is where I
got it: its Decay Rate EQ shapes the decay across the spectrum instead of
splitting it at a crossover.

Three things had to be true for the drawn curve to be the realised one, and
each of them was wrong first:

**The curve has to be shaped like something the filters can make.** Combining
bands by multiplying the decay time asks for an exponential in each band's
shape, and a filter cascade adds in decibels. Written in terms of loss instead
— the bands combining in the reciprocal — the target becomes a sum of filter
shapes. That one change took the worst error from 0.21 octaves of decay time
to 0.02.

**The fit has to care about the right error.** Decay time goes as one over the
loss, so a hundredth of a decibel at the bottom of the band is seconds. An
unweighted fit spends its accuracy at the top and leaves 12 s asked for reading
17.7 s at 30 Hz.

**The filters have to be able to hold the numbers.** A direct-form biquad
cannot express a hundredth-of-a-decibel loss at 20 Hz: its coefficients are the
difference of numbers near two, and rounding them to `f32` moves the answer by
more than the answer is. They are state variable filters now, which removed the
sample-rate dependence entirely.

## Measured

[`docs/BENCHMARK.md`](docs/BENCHMARK.md) is generated by the benchmark binary,
and every row in it is measured from **rendered audio** — an impulse through
the engine, the tail filtered into octaves, the decay read by integrating the
energy backwards. Nothing in it asks the filters what they meant to do.

| what was drawn | worst error across seven octave bands |
|---|---|
| flat, 2 s | 0.014 octaves of decay time |
| lows ×1.5, highs ×0.7 | 0.023 |
| lows ×2, highs ×0.5 | 0.056 |
| lows ×4, highs ×0.25 | 0.187, at the shelf's corner |

The last row said **0.455** until the measurement was fixed rather than the
engine. Its band-pass had two sections, and on a hard bend the neighbouring
band's much longer energy leaked through the skirts and became what the fit
read. With four sections the curve figures roughly halved and the plate and
spring figures did not move at all — which is the useful part: a better ruler
separated the engine's real limits from the ruler's own.

Every network mode measures its own decay to within 0.08 octaves and most to
within 0.02. **The plate and the spring are looser, near 0.3**, and the reason
is structural rather than a matter of tuning: their loops run through
allpasses, whose delay depends on frequency, while the loss is fitted to a
single number for one lap. No single number can express a delay that varies. I
have published what is left rather than fitting a correction factor to the
measurements, which would improve the number without improving the tank.

## What is in it

The band shapes — bell, low and high shelf, notch and **tilt** — come from
[noob-band-shapes](https://github.com/Noob-Audio-Engineering/noob-band-shapes),
shared with Noob-Q. An equaliser wants a filter's actual response and a reverb
wants the same shape with the gain divided out, because one drawn curve here
becomes a different filter on every delay line; the crate holds both and a test
holds them to being the same shape in the limit. It caught the two plug-ins
disagreeing about which way a tilt points on the first comparison.

**Four architectures**, which are the real structural differences:

- a **feedback delay network** of up to sixteen lines, for halls, rooms,
  chambers, clouds and echoes;
- a **plate**, Dattorro's figure of eight, carrying a fitted loss instead of a
  damping filter, so a plate's decay can be drawn like a hall's;
- a **spring**, a hundred allpasses in cascade, which chirps rather than
  diffuses;
- **early reflections** from the image sources of a shoebox, which say where
  the walls are.

**The axes the survey found the modes actually vary on** — attack, decay,
density, size — are controls rather than presets of a hidden algorithm.
Twenty-nine modes are places to stand in that space, and everything a mode sets
stays movable afterwards. Twelve presets go further, each a mode plus the
handful of controls it moves; they are stored that way rather than as a dump of
every parameter, because a dump has to be re-issued whenever a parameter is
added and silently restores a default over something you set.

**Modulation** is two things on one control: a smooth sweep, which detunes the
tail, and a random walk, which loosens the same ringing without moving the
pitch. **Era** is a bandwidth, a word length and a rate inside the tank,
applied to whichever architecture is running. **Shape** is the machines that
are not spaces at all — gate, reverse, ramp, swoosh, swell. **Shift** puts a
transposed copy back into the feedback, which is what a shimmer is.

**Distance** and **Thickness** are macros and say so: distance moves the early
pattern down, the diffusion up, the build-up longer and the top down a little,
because that is what walking away from a source does and every one of those is
a control that already exists. Thickness is density and a bounded saturation
into the tank, so driving it harder makes the reverb denser and cannot make it
louder without limit — which matters when the thing being driven is a feedback
loop.

**Ducking** and the **auto gate** both watch the dry signal rather than the
wet. Ducking from the wet is a compressor on the return: the tail pulls itself
down and releases into its own decay, which pumps. The gate's threshold follows
the signal, so the same performance printed twelve decibels quieter gates the
same way — measured.

**Bloom** is a generator with its own rising feedback in front of the tank, so
the reverb arrives *after* the note rather than being faded in behind it, which
is what Attack does. Measured on held noise, it builds 1.47× while the same
path without it stays flat at 0.98×.

**Tape** is a multi-head echo *in front of* the tank, which is what makes it a
Magneto rather than an echo after a reverb: its repeats are what the room
hears, so each one gets its own tail. The heads sit at unequal fractions of one
circulating delay, wow and flutter move the read rather than the write, and
every pass goes through saturation and a bandwidth limit, so a repeat gets
darker as it goes rather than only quieter.

**Choir** puts a vowel's formants on the tail. A vowel is two or three
resonances at *fixed* frequencies — they do not move with the pitch, which is
why a sung note stays the same vowel across an octave — so it is a bank of
fixed filters on the late energy rather than anything tracking the input.

## What this does not claim

**I have not measured Valhalla's plug-ins, FabFilter's Pro-R or Strymon's
BigSky.** [`docs/SURVEY.md`](docs/SURVEY.md) is what I read of their own public
documentation before writing any code, quoted with the source named, and what I
took from each. Nothing in this repository claims a margin over any of them in
any unit, and the reason is simply that no such measurement exists to support
one.

What this plug-in claims is about itself: that the decay you draw at a
frequency is the decay the tail has at that frequency, and `docs/BENCHMARK.md`
is where that is checked, including where it is worst.

## Licence

MIT or Apache-2.0, at your option.
