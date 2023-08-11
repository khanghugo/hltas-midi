# hltas-midi

Generates a .hltas file from a .mid file to "play the midi" in-game by creating pitch from sounds in Half-Life games.
Requires ![BunnymodXT](https://github.com/YaLTeR/BunnymodXT) to load the script and ![bxt-rs](https://github.com/YaLTeR/bxt-rs) to record video.

![Example](https://youtu.be/fbyGEyvEn4c)

Yes polyphonic.

It is just a script so you can modify it.

# How To

Each staff will correspond to each member of `sounds` vector in `main()` function.

[https://github.com/khanghugo/hltas-midi/blob/36ab6c6e1ca1ced9b57cc1f1af46a9214be13f95/src/main.rs#L221](https://github.com/khanghugo/hltas-midi/blob/36ab6c6e1ca1ced9b57cc1f1af46a9214be13f95/src/main.rs#L221-L224)https://github.com/khanghugo/hltas-midi/blob/36ab6c6e1ca1ced9b57cc1f1af46a9214be13f95/src/main.rs#L221-L224

Take a look at provided examples of simplied MuseScore files if there's any confusion. 

For chords, it's better to separate each note into another staff. Otherwise, the note with highest frequency will be selected (or not depending on MIDI export). 
