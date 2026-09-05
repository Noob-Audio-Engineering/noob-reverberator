# What the reverbs I admire actually do

A survey I ran before writing any code, so the shape of this plug-in is a
response to something rather than a guess. Everything here is quoted or
paraphrased from the makers' own public pages, with the source named. Where I
could not find a number I say so instead of inventing one.

**None of this is a comparison.** I have not measured any of these products,
and nothing in this repository claims a margin over any of them. What the
survey is for is deciding *which questions a reverb is expected to answer*, so
that mine answers them too, and so that the places where mine does something
different are deliberate rather than accidental.

## Valhalla DSP

### ValhallaVintageVerb --- 22 algorithms and 3 "colours"

The mode list, verbatim from the product page: Concert Hall, Bright Hall,
Plate, Room, Chamber, Random Space, Chorus Space, Ambience, Sanctuary, Dirty
Hall, Dirty Plate, Smooth Plate, Smooth Room, Smooth Random, Nonlin, Chaotic
Hall, Chaotic Chamber, Chaotic Neutral, Cathedral, Palace, Chamber1979,
Hall1984.

The COLOR control is the interesting structural idea: three eras that change
the *bandwidth and the artefacts*, not the topology. The 1970s colour
"replicates the reduced bandwidth of the earliest digital reverberators (10 kHz
maximum output frequency)" and is "downsampled internally to reproduce the
artifacts of running at a lower sampling rate"; 1980s is full bandwidth with
the same noisy modulation; NOW is "clean and colorless".

What I take from it: **era is a separate axis from architecture.** A hall and a
plate can both be run through the same converter-and-bandwidth colouring, and
that is one control rather than forty extra algorithms.

The mode descriptions repeatedly distinguish two kinds of modulation, and the
distinction matters because they sound different and cost different amounts:

- **chorused** --- delay lengths swept by a smooth LFO, which detunes the tail;
- **randomised** --- delay lengths jittered, described as reducing "metallic
  artifacts without the pitch change that can occur in the algorithms with
  chorused modulation" (Random Space).

Several modes are explicitly about *echo density over time* rather than decay:
Ambience "combines time varying randomized early reflections with a
full-featured reverb tail, with the balance between early and late reverb
controlled by the Attack knob". Nonlin uses Attack to "smoothly interpolate
between a truncated reverb, a 'flat' gated decay, and huge reverse reverbs".

### ValhallaRoom --- twelve algorithms, described psychoacoustically

Named modes include Large Room, Large Chamber, Bright Room, Dark Room, Dark
Chamber, Dark Space, Nostromo, Narcissus, Sulaco and LV-426. The design note is
the part worth copying: it is "designed from a psychoacoustic perspective.
Instead of creating a simplified physical model of a simplified physical space,
Room generates early and late acoustic energy that provides the spatial and
phase cues needed to create an 'idealized' room impression."

The per-mode notes name the axes the algorithms actually differ on: initial
echo density (sparse to dense), how fast that density builds, modal density,
attack time, stereo width, and whether the modulation causes pitch shift. Large
Room is called "shoebox"; Dark Room is a "square" geometry with more random
modulation that "can cause some random pitch shifts for long decays".

### ValhallaSupermassive --- 22 modes, and it is free

Modes: Gemini, Hydra, Centaurus, Sagittarius, Great Annihilator, Andromeda,
Lyra, Capricorn, Large Magellanic Cloud, Triangulum, Cirrus Major, Cirrus
Minor, Cassiopeia, Orion, Aquarius, Pisces, Scorpio, Libra, Leo, Virgo,
Pleiades, Sirius.

Every description is a point in the same three-dimensional space: **attack
speed, decay length, echo density** --- Gemini is "fast attack, shorter decay,
high echo density"; Andromeda is "slowest attack, very long decay, very high
echo density". Several later modes add "filtering in the feedback loop for more
realistic reverb decay", and several are explicitly echo/reverb hybrids
("a dedicated EchoVerb algorithm, with a strong audible echo/delay that can
morph into a lush reverb with higher DENSITY settings").

What I take from it: those three axes deserve to be **controls**, not
twenty-two presets of a hidden topology. A mode list is a good way to *ship*
tuned points in that space; it is a poor way to be the only way to reach them.

### ValhallaShimmer

Its reverb modes are sizes rather than architectures --- Mono, Small Stereo,
Medium Stereo, Big Stereo --- with pitch shifting in the feedback path as the
point of the plug-in.

## FabFilter Pro-R 2

Controls: Space, Decay Rate, Style, Predelay, Character, Brightness, Distance,
Thickness, Ducking, Auto Gate, Stereo Width, Freeze and Mix.

The figures FabFilter publish:

| control | what they say |
|---|---|
| Space | one knob spanning room model and decay, "from a very small ambience space (200 ms) to a cathedral-like ten-second reverb", over a dozen room models |
| Decay Rate | "25% to 400% of the decay that corresponds to the current Space setting" |
| Style | Modern, Vintage, Plate |
| Predelay | free, or synced to Quarter/8th/16th/32nd, with an offset "from 50% to 200% of the synchronized time" |
| Character | 0% transparent to 100%, "gradually introducing modulation and more pronounced early reflections" |
| Distance | 0% is close; higher gives "longer build-up and a more diffuse tail" |
| Stereo Width | 0% mono, 50% maximum cross-feeding, 100% true stereo, above 100% amplifies the side signal |

**The Decay Rate EQ is the idea worth taking seriously.** Up to six bands ---
bell, low and high shelf, and notch --- that "freely shape the decay rate over
the frequency spectrum", rather than the low/high crossover pair most reverbs
give you. Their justification is acoustic: high frequencies "often decay quite
fast while bass frequencies stick around much longer".

I could not find published ranges for the decay multiplier, the frequency or
the Q of those bands, so I am not quoting any.

## Strymon BigSky

Twelve machines: Hall, Plate, Spring, Swell, Bloom, Cloud, Chorale, Shimmer,
Magneto, Nonlinear, Reflections, Room. Knobs: Decay, Pre-Delay (0--1.5
seconds), Mix, Tone, Mod, and two per-machine assignable parameters.

The machines are the widest net of the three surveys, because several are not
"a room" at all:

- **Swell** --- the reverb is faded in behind the dry signal;
- **Bloom** --- a "bloom generating" section with its own feedback feeds the tank;
- **Chorale** --- vowel-filtered voices, a choir rather than a space;
- **Shimmer** --- "two tunable voices add pitch-shifted tones";
- **Magneto** --- a multi-head tape echo whose pre-delay has feedback and diffusion;
- **Nonlinear** --- "physics-defying shapes including Swoosh, Reverse, Ramp, and Gate";
- **Reflections** --- "psycho-acoustically accurate small-space reverb calculating 250 room reflections".

What I take from it: **two assignable parameters per machine is an admission
that the machines are different instruments.** On a pedal, with a fixed panel,
that is the only option available. In a window that can draw anything it is
not, and hiding a machine's real controls behind Param 1 and Param 2 would be
copying a constraint rather than a design.

## What this tells me to build

1. **Architecture and era are separate axes.** Valhalla's COLOR shows one
   colouring stage can serve every topology, so era is a control rather than a
   duplicate of every algorithm.
2. **Attack, decay and echo density are the axes the modes actually vary.**
   Supermassive's own descriptions are written in them. They should be
   controls, with the named modes as tuned points in that space rather than
   the only way to reach it.
3. **Decay against frequency wants a curve, not a crossover.** Pro-R's Decay
   Rate EQ is right, and it maps exactly onto per-line loss filters in a
   feedback delay network: fit each line's filter so one trip round the loop
   loses the right number of decibels at each frequency.
4. **A reverb is not only rooms.** Shimmer, chorale, magneto, nonlinear and
   bloom are structurally different enough to need their own controls.
5. **Nothing here claims to beat any of them.** I have measured none of these
   products. What I can measure is whether *this* one does what its own
   controls say --- and the thing worth measuring is point 3: **does the decay
   time you draw against frequency turn up in the tail?** That is the row this
   plug-in's benchmark is about.

## Sources

- Valhalla DSP, ValhallaVintageVerb: <https://valhalladsp.com/shop/reverb/valhalla-vintage-verb/>
- Valhalla DSP, ValhallaRoom: <https://valhalladsp.com/shop/reverb/valhalla-room/>
- Valhalla DSP, ValhallaSupermassive: <https://valhalladsp.com/shop/reverb/valhalla-supermassive/>
- Valhalla DSP, ValhallaShimmer: <https://valhalladsp.com/shop/reverb/valhalla-shimmer/>
- FabFilter, Pro-R 2 help --- overview, main controls, Decay Rate EQ:
  <https://www.fabfilter.com/help/pro-r/using/overview>,
  <https://www.fabfilter.com/help/pro-r/using/maincontrols>,
  <https://www.fabfilter.com/help/pro-r/using/decayrateEQ>
- Strymon, BigSky: <https://www.strymon.net/product/bigsky/>

Fetched 5 September 2026.
