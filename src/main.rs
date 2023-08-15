use std::{f64, format, fs::File, io::Write};

use itertools::Itertools;
use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};

fn pitch_frequency(key: u8) -> f64 {
    // A4 is 69 and index will be reported u7(value)
    440f64 * f64::powf(2f64, f64::from(key as i32 - 69) / 12f64)
}

fn pitch_to_frametime(key: u8) -> f64 {
    1f64 / pitch_frequency(key)
}

fn midi_tick_to_duration(tempo: u32, tick: u32) -> f64 {
    // tempo is micro-second / quarter note
    // a quarter node is 480 ticks
    // duration will be in second
    f64::from(tempo) / 1_000_000f64 / 480f64 * f64::from(tick)
}

fn frametime_tick_to_repeat(tempo: u32, frametime: f64, tick: u32) -> u32 {
    // For monophonic or a note that is too long playing alone
    // Used for monophonic or a really long note
    (midi_tick_to_duration(tempo, tick) / frametime) as u32
}

#[derive(Clone, PartialEq)]
struct EmitInfo {
    sound: String,
    channel: i32,
    volume: f32,
    from: u32,
}

#[derive(Clone, PartialEq)]
enum Command {
    None,
    /// `impulse 101`
    Flashlight,
    /// `slot3; slot3; slot3;...`
    SwitchScroll(u8),
    /// `slot3; slot2; slot3;...`
    SwitchGroup,
    /// `+use; wait; -use`
    Use,
    /// Ducktap in combination of `sv_gravity 11550`, `sv_gravity 11650`, or bogus with `god`  
    Ducktap,
    ///
    Nice,
    Nice2,
    Nice3,
    Stopsound,
    Attack1,
    WpnMoveSelect,
    Emit(EmitInfo),
    EmitDynamic(EmitInfo),
    PauseFinder,
}
static mut COUNT: usize = 0;
static mut SWITCH: u8 = 1;
fn format_bulk(frametime: f64, repeat: u32, command: Command) -> String {
    match command {
        Command::Emit(EmitInfo {
            sound,
            channel,
            volume,
            from,
        }) => format!(
            // bxt_emit_sound "common/bodysplat.wav 0 255 0 0 0.8 0 100"
            // Usage: bxt_emit_sound <sound> <channel> [volume] [from] [to] [attenuation] [flag] [pitch]
            "----------|------|------|{}|-|-|{}|bxt_emit_sound \"{} {} {} {} 0 0.8 0 100\"\n",
            frametime, repeat, sound, channel, volume, from
        ),
        Command::EmitDynamic(EmitInfo {
            sound,
            channel,
            volume,
            from,
        }) => format!(
            // bxt_emit_sound_dynamic "common/bodysplat.wav 0 255 0 0 0.8 0 100"
            // Usage: bxt_emit_sound <sound> <channel> [volume] [from] [to] [attenuation] [flag] [pitch]
            "----------|------|------|{}|-|-|{}|bxt_emit_sound_dynamic \"{} {} {} {} 0 0.8 0 100\"\n",
            frametime, repeat, sound, channel, volume, from
        ),
        Command::Ducktap => format!("-----d----|------|------|{}|-|-|{}\n", frametime, repeat),
        Command::Use => format!("----------|------|--u---|{}|-|-|{}\n", frametime, repeat),
        Command::PauseFinder => {
            format!("----------|------|------|{}|-|-|{}|echo {}\n", frametime, repeat, {
                unsafe {COUNT = COUNT + 1; COUNT}
            })
        }
        _ => format!(
            "----------|------|------|{}|-|-|{}|{}\n",
            frametime,
            repeat,
            match command {
                Command::None => "",
                Command::Flashlight => "impulse 100",
                Command::Nice => "speak player/sprayer",
                Command::Nice2 => "speak \"common/bodysplat(v30)\"",
                Command::Nice3 => "speak \"common/wpn_moveselect(v30)\"",
                Command::SwitchScroll(num) => match num {
                    0 => "slot0",
                    1 => "slot1",
                    2 => "slot2",
                    3 => "slot3",
                    4 => "slot4",
                    5 => "slot5",
                    _ => "",
                },
                Command::SwitchGroup => {
                    unsafe {
                        SWITCH = SWITCH % 2 + 1;
                        match SWITCH {
                            1 => "slot1",
                            2 => "slot2",
                            _ => "slot0",
                        }
                    }
                }
                Command::Stopsound => "stopsound",
                Command::Attack1 => "+attack; wait; -attack",
                Command::WpnMoveSelect => "speak \"common/wpn_moveselect\"",
                _ => "",
            }
        ),
    }
}

fn print_events(smf: &Smf) {
    for (i, track) in smf.tracks.iter().enumerate() {
        println!("Track {}", i);
        for (j, event) in track.iter().enumerate() {
            println!("{j} : {:?}", event);
        }
    }
}

#[derive(Debug, Clone)]
struct TrackSegment {
    read_idx: usize,
    end: bool,
    key: u8,
    vel: u8,
    tick: u32,
    // stores how many seconds left until the note is done playing
    remainder: f64,
    // stores the frametime difference between current note and the next one
    quantize_remainder: f64,
    new_beat: bool,
}

impl TrackSegment {
    const fn new() -> Self {
        TrackSegment {
            read_idx: 0,
            end: false,
            key: 0,
            vel: 0,
            tick: 0,
            remainder: 0.,
            quantize_remainder: 0.,
            new_beat: false,
        }
    }
}
fn main() {
    // EDIT HERE
    let smf = Smf::parse(include_bytes!(
        // Edit file name here.
        "../examples/smw_athletic_theme_pal_grunt2.mid"
    ))
    .unwrap();

    // Edit sound font here.
    let sounds: Vec<Command> = vec![
        // Command::Emit(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 7, volume: 0.4, from: 0}),
        // Command::Emit(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 1, volume: 0.3, from: 0}),
        // Command::Emit(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 2, volume: 0.2, from: 0}),
        // Command::Emit(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 3, volume: 0.1, from: 68}),
        // Command::Emit(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 4, volume: 0.1, from: 116}),
        Command::SwitchScroll(2),
        // Command::None,
        Command::Nice3,
        // Command::EmitDynamic(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 6, volume: 0.3, from: 37}),
        // Command::Emit(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 3, volume: 0.1, from: 35}),
        // Command::Emit(EmitInfo { sound: "common/bodysplat.wav".to_string(), channel: 4, volume: 0.3, from: 36}),
        // Command::Ducktap,
        // Command::Emit(EmitInfo { sound: "player/pl_tile2.wav".to_string(), channel: 4, volume: 150, target: 0}),

        // Command::SwitchScroll(2),
        // Command::Use,
        // Command::Nice2,
        // Command::WpnMoveSelect,
        // Command::Flashlight,
        // Command::SwitchScroll(2),
    ];
    // Print echo after every sound emit to see where it stops due to level change.
    let find_pause = false;
    // Print some logs during conversion.
    let print_midi = false;

    //
    //
    //
    //
    // MAYBE NO NEED TO GO FROM HERE
    const ZEROFRAMETIME: f64 = 0.000000000001;
    let legato = 1; // play notes in succession or have like 0.002s downtime to enounciate, true = saves more line

    let mut file = File::create("foo.txt").unwrap();
    const IJUSTWANTTOREAD: bool = false;
    let mut tempo = 0;

    if print_midi {
        print_events(&smf);
    }

    let mut curr_track = 0;
    let mut track_segments: Vec<TrackSegment> = Vec::new();
    for _ in 0..smf.tracks.len() {
        track_segments.push(TrackSegment::new());
    }

    // If it doesn't match, panic right away.
    if sounds.len() != track_segments.len() {
        panic!(
            "Mismatch number of selected sound ({}) and number of available tracks. ({})",
            sounds.len(),
            track_segments.len()
        );
    }

    while !track_segments.iter().fold(true, |acc, e| acc && e.end) && !IJUSTWANTTOREAD {
        let curr = &mut track_segments[curr_track];

        // Subarray of read_idx to continue reading from previous break
        for event in &smf.tracks[curr_track][curr.read_idx..] {
            // Note is still being quantized. Move onto the next track.
            // Make sure to include quantize remainder.
            if curr.remainder > 0. {
                break;
            }

            curr.read_idx += 1;
            curr.new_beat = true;

            match event.kind {
                TrackEventKind::Meta(message) => match message {
                    MetaMessage::Tempo(temp) => tempo = u32::from(temp),
                    MetaMessage::EndOfTrack => {
                        curr.end = true;
                    }
                    _ => (),
                },
                TrackEventKind::Midi {
                    channel: _, // Ignore this since we already know which track we are looking at from the loop.
                    message,
                } => match message {
                    MidiMessage::NoteOn { key, vel } => {
                        if vel > 0 {
                            curr.key = u8::from(key);
                        }
                        curr.vel = u8::from(vel);
                    }
                    _ => (),
                },
                _ => (),
            }

            if event.delta > (0 + legato as u32) {
                // Read until there is delta, which now starts to break into hltas for the segment.
                curr.tick = u32::from(event.delta);
                curr.remainder = midi_tick_to_duration(tempo, curr.tick);
                break;
            }
        }

        curr_track = (curr_track + 1) % smf.tracks.len();

        // The first time running needs to get all of the track read first
        if !track_segments
            .iter()
            .fold(true, |acc, e| acc && (e.read_idx != 0))
        {
            continue;
        }

        // This ends the reading and writing if last read completes the read.
        // HACK
        if track_segments.iter().fold(true, |acc, e| acc && e.end) {
            break;
        }

        // Write to hltas.
        // If there is one track is done quantized, loop will break.
        // Then continue reading from the previous left off index.
        while track_segments
            .iter()
            .filter(|e| !e.end && e.remainder <= 0.)
            .count()
            == 0
        {
            // See how many tracks will be worked on.
            // let work_count = track_segments
            //     .iter()
            //     .filter(|e| !e.end && e.vel == 0)
            //     .count();

            // Attempts to add note
            // if work_count != 1 {
            track_segments
                .clone()
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.end && e.key != 0 && e.quantize_remainder <= 0.)
                .for_each(|(i, _)| {
                    if track_segments[i].vel == 0 {
                        // In MIDI, info in an event is applied after delta.
                        // `vel > 0` for that event means it is a rest.
                        if !track_segments[i].new_beat && sounds[i] == Command::Ducktap {
                        } else {
                            if sounds[i] == Command::Attack1 {
                                write!(file, "{}", format_bulk(ZEROFRAMETIME, 1, Command::Attack1))
                                    .unwrap();
                            }
                            write!(file, "{}", format_bulk(ZEROFRAMETIME, 1, sounds[i].clone()))
                                .unwrap();
                        }

                        track_segments[i].quantize_remainder =
                            pitch_to_frametime(track_segments[i].key)
                                .min(track_segments[i].remainder); // Min so it does not play extra when overdue.
                        track_segments[i].new_beat = false;
                    } else {
                        write!(
                            file,
                            "{}",
                            format_bulk(ZEROFRAMETIME, 1, Command::Stopsound)
                        )
                        .unwrap();
                        track_segments[i].quantize_remainder = track_segments[i].remainder;
                    }
                });
            // }
            // After adding a note, it will start finding the note with lowest frame time and play that sound.
            // This loops until the one of any note is fully played, which then prompts the reader to read said track.
            // Safety: loop condition makes sure that `unwrap()` always have a value.
            let lowest_index = track_segments
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.end)
                .sorted_by(|(_, a), (_, b)| a.quantize_remainder.total_cmp(&b.quantize_remainder))
                .nth(0)
                .unwrap()
                .0;

            let common_subtractee = track_segments[lowest_index].quantize_remainder;

            if find_pause {
                write!(
                    file,
                    "{}",
                    format_bulk(ZEROFRAMETIME, 1, Command::PauseFinder)
                )
                .unwrap();
            }

            {
                write!(
                    file,
                    "{}",
                    format_bulk(common_subtractee, 1, Command::None,)
                )
                .unwrap();
            }
            // update
            track_segments = track_segments
                .iter()
                .map(|e| TrackSegment {
                    remainder: e.remainder - common_subtractee,
                    quantize_remainder: e.quantize_remainder - common_subtractee,
                    ..*e
                })
                .collect_vec();

            // if track_segments[lowest_index].quantize_remainder <= 0. {
            //     track_segments[lowest_index].quantize_remainder = 0.;
            // }
        }
    }
}
