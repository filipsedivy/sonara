//! The sources the dataset cites, each written out in full once.
//!
//! An entry names its author or publisher and its title, with the edition for a book, and then
//! where it was read: a URL, or a page for a printed book without one. The sources were read in
//! October 2026.
//!
//! A `Cited` provenance is written with [`cite!`], which expands at compile time to each named
//! source in full followed by what the source states, so the record keeps the statement next to
//! the value and the source's text lives in one place (DES-01). A name that is not listed here
//! does not compile.

/// Defines `source!`, which expands the name of a listed source to its full text, and, for the
/// tests, `SOURCES`: every name with its text, in the order listed.
macro_rules! define_sources {
    ($($name:ident => $text:literal,)+) => {
        /// The full text of a listed source, by name.
        macro_rules! source {
            $(($name) => {
                $text
            };)+
        }
        pub(super) use source;

        /// Every listed source as its name and its full text, in the order listed.
        #[cfg(test)]
        const SOURCES: &[(&str, &str)] = &[$((stringify!($name), $text)),+];
    };
}

/// The text of a `Cited` provenance: `cite!(name: "what it states", ...)` gives each named source
/// in full, then `": "` and what it states; several citations are joined by `"; "`.
/// `cite!(wikipedia_piccolo: "tessitura D5-C8")` is
/// `"Wikipedia, Piccolo, https://en.wikipedia.org/wiki/Piccolo: tessitura D5-C8"`.
macro_rules! cite {
    ($name:ident: $states:literal $(,)?) => {
        concat!(
            $crate::orchestration::data::sources::source!($name),
            ": ",
            $states
        )
    };
    ($name:ident: $states:literal, $($rest:tt)+) => {
        concat!(
            $crate::orchestration::data::sources::source!($name),
            ": ",
            $states,
            "; ",
            $crate::orchestration::data::sources::cite!($($rest)+)
        )
    };
}
pub(super) use cite;

define_sources! {
    // Orchestration handbooks and acoustics texts.
    askenfelt_jansson => "Anders Askenfelt and Erik Jansson, From touch to string vibrations, in \
        Five Lectures on the Acoustics of the Piano (Royal Swedish Academy of Music, 1990), \
        https://www.speech.kth.se/music/5_lectures/askenflt/askenflt.html",
    berlioz => "Hector Berlioz, A Treatise on Modern Instrumentation and Orchestration, \
        translated by Mary Cowden Clarke, revised by Joseph Bennett (Novello, 1882), \
        https://archive.org/details/treatiseonmodern00berl",
    brink_carillon => "Joey Brink, Composing for Carillon (Rockefeller Memorial Chapel, \
        University of Chicago, 2017), \
        https://rockefeller.uchicago.edu/2017/12/19/composing-for-carillon",
    forsyth => "Cecil Forsyth, Orchestration (Macmillan and Stainer & Bell, 1914), Cornell \
        University Library copy, https://archive.org/details/cu31924022381440",
    forsyth_second_scan => "Cecil Forsyth, Orchestration (Macmillan and Stainer & Bell, 1914), \
        https://archive.org/details/orchestration00fors",
    hugill_fretted_strings => "Andrew Hugill with the Philharmonia Orchestra, The Orchestra: A \
        User's Manual, fretted string pages, https://andrewhugill.com/OrchestraManual/frets.html",
    hugill_harps => "Andrew Hugill with the Philharmonia Orchestra, The Orchestra: A User's \
        Manual, harp pages, https://andrewhugill.com/OrchestraManual/harps.html",
    hugill_pitched_percussion => "Andrew Hugill with the Philharmonia Orchestra, The Orchestra: A \
        User's Manual, pitched percussion pages, \
        https://andrewhugill.com/OrchestraManual/percussion_pitchedmetal.html",
    prout_figures => "Ebenezer Prout, The Orchestra, volume 1, chapter VII, figure descriptions \
        by the Timbre and Orchestration Resource (McGill University), \
        https://timbreandorchestration.org/treatises/figures/prout/volume-1/chapter-vii",
    rimsky_korsakov => "Nikolai Rimsky-Korsakov, Principles of Orchestration, edited by \
        Maximilian Steinberg, translated by Edward Agate (1922; Dover reprint, 1964), Project \
        Gutenberg ebook 33900, https://www.gutenberg.org/ebooks/33900",
    widor => "Charles-Marie Widor, The Technique of the Modern Orchestra, translated by Edward \
        Suddard (Joseph Williams, 1906), Cornell University Library copy, \
        https://archive.org/details/cu31924022381556",
    widor_figures => "Charles-Marie Widor, The Technique of the Modern Orchestra (1906), figure \
        captions by the Timbre and Orchestration Resource (McGill University), \
        https://timbreandorchestration.org/treatises/figures/widor",
    widor_second_scan => "Charles-Marie Widor, The Technique of the Modern Orchestra, translated \
        by Edward Suddard (Joseph Williams, 1906), \
        https://archive.org/details/techniqueofmoder00widouoft",

    // Encyclopedias: English Wikipedia, then Wikipedia in other languages.
    wikipedia_accordion => "Wikipedia, Accordion, https://en.wikipedia.org/wiki/Accordion",
    wikipedia_accordion_reed_ranks => "Wikipedia, Accordion reed ranks and switches, \
        https://en.wikipedia.org/wiki/Accordion_reed_ranks_and_switches",
    wikipedia_alto => "Wikipedia, Alto, https://en.wikipedia.org/wiki/Alto",
    wikipedia_alto_flute => "Wikipedia, Alto flute, https://en.wikipedia.org/wiki/Alto_flute",
    wikipedia_alto_saxophone => "Wikipedia, Alto saxophone, \
        https://en.wikipedia.org/wiki/Alto_saxophone",
    wikipedia_baritone_saxophone => "Wikipedia, Baritone saxophone, \
        https://en.wikipedia.org/wiki/Baritone_saxophone",
    wikipedia_bass_clarinet => "Wikipedia, Bass clarinet, \
        https://en.wikipedia.org/wiki/Bass_clarinet",
    wikipedia_bass_drum => "Wikipedia, Bass drum, https://en.wikipedia.org/wiki/Bass_drum",
    wikipedia_bass_flute => "Wikipedia, Bass flute, https://en.wikipedia.org/wiki/Bass_flute",
    wikipedia_bass_trombone => "Wikipedia, Bass trombone, \
        https://en.wikipedia.org/wiki/Bass_trombone",
    wikipedia_bass_voice => "Wikipedia, Bass (voice type), \
        https://en.wikipedia.org/wiki/Bass_(voice_type)",
    wikipedia_bassoon => "Wikipedia, Bassoon, https://en.wikipedia.org/wiki/Bassoon",
    wikipedia_carillon => "Wikipedia, Carillon, https://en.wikipedia.org/wiki/Carillon",
    wikipedia_castanets => "Wikipedia, Castanets, https://en.wikipedia.org/wiki/Castanets",
    wikipedia_celesta => "Wikipedia, Celesta, https://en.wikipedia.org/wiki/Celesta",
    wikipedia_chamber_choir => "Wikipedia, Chamber choir, \
        https://en.wikipedia.org/wiki/Chamber_choir",
    wikipedia_choir => "Wikipedia, Choir, https://en.wikipedia.org/wiki/Choir",
    wikipedia_clapper => "Wikipedia, Clapper (musical instrument), \
        https://en.wikipedia.org/wiki/Clapper_(musical_instrument)",
    wikipedia_clarinet => "Wikipedia, Clarinet, https://en.wikipedia.org/wiki/Clarinet",
    wikipedia_clash_cymbals => "Wikipedia, Clash cymbals, \
        https://en.wikipedia.org/wiki/Clash_cymbals",
    wikipedia_classical_guitar => "Wikipedia, Classical guitar, \
        https://en.wikipedia.org/wiki/Classical_guitar",
    wikipedia_clavichord => "Wikipedia, Clavichord, https://en.wikipedia.org/wiki/Clavichord",
    wikipedia_clavinet => "Wikipedia, Clavinet, https://en.wikipedia.org/wiki/Clavinet",
    wikipedia_contrabassoon => "Wikipedia, Contrabassoon, \
        https://en.wikipedia.org/wiki/Contrabassoon",
    wikipedia_cor_anglais => "Wikipedia, Cor anglais, https://en.wikipedia.org/wiki/Cor_anglais",
    wikipedia_crotales => "Wikipedia, Crotales, https://en.wikipedia.org/wiki/Crotales",
    wikipedia_cymbal => "Wikipedia, Cymbal, https://en.wikipedia.org/wiki/Cymbal",
    wikipedia_double_bass => "Wikipedia, Double bass, https://en.wikipedia.org/wiki/Double_bass",
    wikipedia_e_flat_clarinet => "Wikipedia, E-flat clarinet, \
        https://en.wikipedia.org/wiki/E-flat_clarinet",
    wikipedia_eight_foot_pitch => "Wikipedia, Eight-foot pitch, \
        https://en.wikipedia.org/wiki/Eight-foot_pitch",
    wikipedia_euphonium => "Wikipedia, Euphonium, https://en.wikipedia.org/wiki/Euphonium",
    wikipedia_extended_technique => "Wikipedia, Extended technique, \
        https://en.wikipedia.org/wiki/Extended_technique",
    wikipedia_fach => "Wikipedia, Fach, https://en.wikipedia.org/wiki/Fach",
    wikipedia_flute => "Wikipedia, Flute, https://en.wikipedia.org/wiki/Flute",
    wikipedia_french_horn => "Wikipedia, French horn, https://en.wikipedia.org/wiki/French_horn",
    wikipedia_general_midi => "Wikipedia, General MIDI, \
        https://en.wikipedia.org/wiki/General_MIDI",
    wikipedia_general_midi_level_2 => "Wikipedia, General MIDI Level 2, \
        https://en.wikipedia.org/wiki/General_MIDI_Level_2",
    wikipedia_gong => "Wikipedia, Gong, https://en.wikipedia.org/wiki/Gong",
    wikipedia_growling => "Wikipedia, Growling (wind instruments), \
        https://en.wikipedia.org/wiki/Growling_(wind_instruments)",
    wikipedia_hammond_organ => "Wikipedia, Hammond organ, \
        https://en.wikipedia.org/wiki/Hammond_organ",
    wikipedia_harpsichord => "Wikipedia, Harpsichord, https://en.wikipedia.org/wiki/Harpsichord",
    wikipedia_honky_tonk_piano => "Wikipedia, Honky-tonk piano (redirects to Tack piano), \
        https://en.wikipedia.org/wiki/Honky-tonk_piano",
    wikipedia_humming => "Wikipedia, Humming, https://en.wikipedia.org/wiki/Humming",
    wikipedia_imperial_bosendorfer => "Wikipedia, Imperial Bösendorfer, \
        https://en.wikipedia.org/wiki/Imperial_B%C3%B6sendorfer",
    wikipedia_jingle_bell => "Wikipedia, Jingle bell, https://en.wikipedia.org/wiki/Jingle_bell",
    wikipedia_lute => "Wikipedia, Lute, https://en.wikipedia.org/wiki/Lute",
    wikipedia_mandolin => "Wikipedia, Mandolin, https://en.wikipedia.org/wiki/Mandolin",
    wikipedia_maraca => "Wikipedia, Maraca, https://en.wikipedia.org/wiki/Maraca",
    wikipedia_marimba => "Wikipedia, Marimba, https://en.wikipedia.org/wiki/Marimba",
    wikipedia_mute => "Wikipedia, Mute (music), https://en.wikipedia.org/wiki/Mute_(music)",
    wikipedia_oboe => "Wikipedia, Oboe, https://en.wikipedia.org/wiki/Oboe",
    wikipedia_oboe_d_amore => "Wikipedia, Oboe d'amore, \
        https://en.wikipedia.org/wiki/Oboe_d%27amore",
    wikipedia_organ_stop => "Wikipedia, Organ stop, https://en.wikipedia.org/wiki/Organ_stop",
    wikipedia_pedal_harp => "Wikipedia, Pedal harp, https://en.wikipedia.org/wiki/Pedal_harp",
    wikipedia_piano => "Wikipedia, Piano, https://en.wikipedia.org/wiki/Piano",
    wikipedia_piano_accordion => "Wikipedia, Piano accordion, \
        https://en.wikipedia.org/wiki/Piano_accordion",
    wikipedia_piccolo => "Wikipedia, Piccolo, https://en.wikipedia.org/wiki/Piccolo",
    wikipedia_piccolo_trumpet => "Wikipedia, Piccolo trumpet, \
        https://en.wikipedia.org/wiki/Piccolo_trumpet",
    wikipedia_pipe_organ => "Wikipedia, Pipe organ, https://en.wikipedia.org/wiki/Pipe_organ",
    wikipedia_prepared_piano => "Wikipedia, Prepared piano, \
        https://en.wikipedia.org/wiki/Prepared_piano",
    wikipedia_rhodes_piano => "Wikipedia, Rhodes piano, \
        https://en.wikipedia.org/wiki/Rhodes_piano",
    wikipedia_saxophone => "Wikipedia, Saxophone, https://en.wikipedia.org/wiki/Saxophone",
    wikipedia_saxophone_technique => "Wikipedia, Saxophone technique, \
        https://en.wikipedia.org/wiki/Saxophone_technique",
    wikipedia_shaker => "Wikipedia, Shaker (musical instrument), \
        https://en.wikipedia.org/wiki/Shaker_(musical_instrument)",
    wikipedia_snare_drum => "Wikipedia, Snare drum, https://en.wikipedia.org/wiki/Snare_drum",
    wikipedia_sonatas_and_interludes => "Wikipedia, Sonatas and Interludes, \
        https://en.wikipedia.org/wiki/Sonatas_and_Interludes",
    wikipedia_soprano_saxophone => "Wikipedia, Soprano saxophone, \
        https://en.wikipedia.org/wiki/Soprano_saxophone",
    wikipedia_stradella_bass => "Wikipedia, Stradella bass system, \
        https://en.wikipedia.org/wiki/Stradella_bass_system",
    wikipedia_string_section => "Wikipedia, String section, \
        https://en.wikipedia.org/wiki/String_section",
    wikipedia_suspended_cymbal => "Wikipedia, Suspended cymbal, \
        https://en.wikipedia.org/wiki/Suspended_cymbal",
    wikipedia_swell_box => "Wikipedia, Swell box, https://en.wikipedia.org/wiki/Swell_box",
    wikipedia_symphonic_choirs => "Wikipedia, Symphonic Choirs, \
        https://en.wikipedia.org/wiki/Symphonic_Choirs",
    wikipedia_tambourine => "Wikipedia, Tambourine, https://en.wikipedia.org/wiki/Tambourine",
    wikipedia_temple_blocks => "Wikipedia, Temple blocks, \
        https://en.wikipedia.org/wiki/Temple_blocks",
    wikipedia_tenor => "Wikipedia, Tenor, https://en.wikipedia.org/wiki/Tenor",
    wikipedia_tenor_saxophone => "Wikipedia, Tenor saxophone, \
        https://en.wikipedia.org/wiki/Tenor_saxophone",
    wikipedia_timpani => "Wikipedia, Timpani, https://en.wikipedia.org/wiki/Timpani",
    wikipedia_tonewheel => "Wikipedia, Tonewheel, https://en.wikipedia.org/wiki/Tonewheel",
    wikipedia_triangle => "Wikipedia, Triangle (musical instrument), \
        https://en.wikipedia.org/wiki/Triangle_(musical_instrument)",
    wikipedia_trombone => "Wikipedia, Trombone, https://en.wikipedia.org/wiki/Trombone",
    wikipedia_trumpet => "Wikipedia, Trumpet, https://en.wikipedia.org/wiki/Trumpet",
    wikipedia_tuba => "Wikipedia, Tuba, https://en.wikipedia.org/wiki/Tuba",
    wikipedia_tubular_bells => "Wikipedia, Tubular bells, \
        https://en.wikipedia.org/wiki/Tubular_bells",
    wikipedia_vibraphone => "Wikipedia, Vibraphone, https://en.wikipedia.org/wiki/Vibraphone",
    wikipedia_violin => "Wikipedia, Violin, https://en.wikipedia.org/wiki/Violin",
    wikipedia_voice_type => "Wikipedia, Voice type, https://en.wikipedia.org/wiki/Voice_type",
    wikipedia_wagner_tuba => "Wikipedia, Wagner tuba, https://en.wikipedia.org/wiki/Wagner_tuba",
    wikipedia_western_concert_flute => "Wikipedia, Western concert flute, \
        https://en.wikipedia.org/wiki/Western_concert_flute",
    wikipedia_woodblock => "Wikipedia, Woodblock (instrument), \
        https://en.wikipedia.org/wiki/Woodblock_(instrument)",
    wikipedia_wurlitzer_electronic_piano => "Wikipedia, Wurlitzer electronic piano, \
        https://en.wikipedia.org/wiki/Wurlitzer_electronic_piano",
    wikipedia_xylophone => "Wikipedia, Xylophone, https://en.wikipedia.org/wiki/Xylophone",
    wikipedia_cs_hlasovy_obor => "Czech Wikipedia, Hlasový obor, \
        https://cs.wikipedia.org/wiki/Hlasov%C3%BD_obor",
    wikipedia_cs_pozoun => "Czech Wikipedia, Pozoun, https://cs.wikipedia.org/wiki/Pozoun",
    wikipedia_cs_trubka => "Czech Wikipedia, Trubka, https://cs.wikipedia.org/wiki/Trubka",
    wikipedia_de_oboe_d_amore => "German Wikipedia, Oboe d’amore, \
        https://de.wikipedia.org/wiki/Oboe_d%E2%80%99amore",
    wikipedia_de_piccolotrompete => "German Wikipedia, Piccolotrompete, \
        https://de.wikipedia.org/wiki/Piccolotrompete",
    wikipedia_fr_bugle => "French Wikipedia, Bugle, https://fr.wikipedia.org/wiki/Bugle",

    // Instrument makers, dealers, collections and specialist sites.
    bosendorfer_290_imperial => "Bösendorfer, Concert Grand 290 Imperial, \
        https://www.boesendorfer.com/en/pianos/pianos/Concert-Grand-290-Imperial",
    buffet_crampon_tosca => "Buffet Crampon, Tosca collection (bass clarinet BC1195-2-0, \
        descending to C), https://www.buffetcrampon.com/en-at/collections/tosca",
    electric_druid_hammond => "Tom Wiltshire (Electric Druid), Technical aspects of the Hammond \
        Organ, https://electricdruid.net/?p=316",
    gcna_carillons => "Guild of Carillonneurs in North America, About carillons, \
        https://gcna.org/about-carillons",
    heckel_contrabassoon => "Heckel, Contrabassoon, \
        https://heckel.de/en/instrumente/contrabassoon/",
    hogwood_clavichords => "The Hogwood Collection, clavichords, \
        https://www.hogwood.org/inst-clavichord.htm",
    hohner_clavinet_d6_manual => "Hohner, Clavinet D6 Owner's Manual, transcription on \
        clavinet.com, https://clavinet.com/d6man.php",
    musser_m55b_pro_vibe => "Conn-Selmer, Musser M55B Pro-Vibe vibraphone, \
        https://connselmer.com/instruments/mallet-instruments/musser-pro-vibe-vibraphone-m55b",
    rhodes_service_manual => "Rhodes Service Manual, transcription on fenderrhodes.com, \
        http://www.fenderrhodes.com/org/manual/toc.html",
    schiedmayer_celesta => "Schiedmayer Celesta, All about the celesta and the celesta models, \
        https://www.celesta-schiedmayer.de/en/celesta/all-about-the-celesta/",
    steinert_piano_actions => "M. Steinert & Sons, The Grand vs. Upright piano action, \
        https://msteinert.com/blog/grand-vs-upright-piano-actions",
    wagner_tuba_com_range => "wagner-tuba.com, Physical properties and range, \
        https://www.wagner-tuba.com/wagner-tuba/physical-properties-range/",
    wurlitzer_service_manual => "The Wurlitzer Company, Wurlitzer Electronic Pianos Service \
        Manual, Models 200, 203, 203W, 206, 207, 207V, 214 and 214V, \
        https://static1.squarespace.com/static/581b462f5016e14ae76bd275/t/5ebb550ed544905f3db1d03a/1589335321798/wurlitzer-200-200a-service-manual.pdf",
    yale_hoffmann_clavichord => "Yale Collection of Musical Instruments, clavichord by C. G. \
        Hoffmann (Ronneburg, 1784), https://music.yale.edu/browse-collection/clavichord-49451960",
    yamaha_brass_family => "Yamaha Music Hub, The Brass Family, Explained, \
        https://hub.yamaha.com/winds/brass/the-brass-family-explained/",
    yamaha_cx_grand_pianos => "Yamaha Australia, CX series grand pianos, specifications, \
        https://au.yamaha.com/en/products/musical_instruments/pianos/grand_pianos/cx_series/specs.html",
    yamaha_glockenspiels => "Yamaha USA, glockenspiel specifications (DG-1590A, YG-1210, \
        YG-2500), \
        https://usa.yamaha.com/products/musical_instruments/percussion/glockenspiel/dg-1590a/specs.html",
    yamaha_marimbas => "Yamaha USA, marimba specifications (YM-6100 and the other YM models), \
        https://usa.yamaha.com/products/musical_instruments/percussion/marimbas/ym-6100/specs.html",
    yamaha_timpani_tp8300r => "Yamaha USA, TP-8300R series timpani, lineup, \
        https://usa.yamaha.com/products/musical_instruments/percussion/timpani/tp-8300r/lineup.html",
    yamaha_trombone_guide => "Yamaha Musical Instrument Guide, Trombone: Playing a tenorbass \
        trombone or bass trombone, \
        https://www.yamaha.com/en/musical_instrument_guide/trombone/play/play003.html",
    yamaha_u_upright_pianos => "Yamaha Australia, U series upright pianos, specifications, \
        https://au.yamaha.com/en/products/musical_instruments/pianos/upright_pianos/u_series/specs.html",
    yamaha_vibraphones => "Yamaha USA, vibraphone specifications (YV-2700, YV-3710, YV-3910, \
        YV-4110), \
        https://usa.yamaha.com/products/musical_instruments/percussion/vibraphones/yv-2700/specs.html",
    yamaha_xylophones => "Yamaha USA, xylophone specifications (YX-500R, YX1030PR, YX2035PR), \
        https://usa.yamaha.com/products/musical_instruments/percussion/xylophones/yx-500r_f/specs.html",
    yamaha_ycl_622ii => "Yamaha USA, YCL-622II bass clarinet to low C, specifications, \
        https://usa.yamaha.com/products/musical_instruments/winds/clarinets/ycl-622ii/specs.html",

    // General MIDI tables and notation software data.
    finale_2010_gm_table => "MakeMusic, Finale 2010 user manual, General MIDI Table, \
        https://usermanuals.finalemusic.com/Finale2010Mac/Content/Finale/General_MIDI_Table.htm",
    finale_2012_gm_patches => "MakeMusic, Finale 2012 user manual, General MIDI Patch Numbers & \
        Names, \
        https://usermanuals.finalemusic.com/Finale2012Win/Content/Finale/General_MIDI_Patch_Numbers_F_Names.htm",
    midi_gm1_sound_set_2012 => "MIDI Manufacturers Association, GM1 Sound Set (Wayback Machine \
        copy of 2012), \
        https://web.archive.org/web/2012id_/http://www.midi.org/techspecs/gm1sound.php",
    midi_gm1_sound_set_2021_01 => "MIDI Association, GM 1 Sound Set (Wayback Machine copy of \
        2021-01-25), \
        https://web.archive.org/web/20210125203751/https://www.midi.org/specifications-old/item/gm-level-1-sound-set",
    midi_gm1_sound_set_2021_10 => "MIDI Association, GM 1 Sound Set (Wayback Machine copy of \
        2021-10-18), \
        https://web.archive.org/web/20211018235158/https://www.midi.org/specifications-old/item/gm-level-1-sound-set",
    midi_gm1_specification => "MIDI Manufacturers Association, General MIDI System Level 1 \
        (RP-003), linked from https://midi.org/general-midi-level-1, \
        https://drive.google.com/file/d/1TTM2HI2JAp_XiTJYEJ8gPNvuJpIjJiTP/view",
    musescore_alto_saxophone => "MuseScore, instrument data: Alto Saxophone, \
        https://musescore.org/en/instruments/alto-saxophone",
    musescore_baritone_saxophone => "MuseScore, instrument data: Baritone Saxophone, \
        https://musescore.org/en/instruments/baritone-saxophone",
    musescore_bb_cornet => "MuseScore, instrument data: Bb Cornet, \
        https://musescore.org/en/instruments/bb-cornet",
    musescore_bb_piccolo_trumpet => "MuseScore, instrument data: Piccolo Trumpet in Bb, \
        https://musescore.org/en/instruments/bb-piccolo-trumpet",
    musescore_c_trumpet => "MuseScore, instrument data: C Trumpet, \
        https://musescore.org/en/instruments/c-trumpet",
    musescore_flugelhorn => "MuseScore, instrument data: Flugelhorn, \
        https://musescore.org/en/instruments/flugelhorn",
    musescore_soprano_saxophone => "MuseScore, instrument data: Soprano Saxophone, \
        https://musescore.org/en/instruments/soprano-saxophone",
    musescore_tenor_saxophone => "MuseScore, instrument data: Tenor Saxophone, \
        https://musescore.org/en/instruments/tenor-saxophone",
    pg_music_gm_lists => "PG Music, General MIDI instrument and drum kit lists, \
        https://www.pgmusic.com/tutorial_gm.htm",
    printmusic_2010_gm_table => "MakeMusic, PrintMusic 2010 user manual, General MIDI Table, \
        https://usermanuals.finalemusic.com/PrintMusic2010Mac/Content/General_MIDI_Table.htm",

    // Sample library makers, their manuals, DAW guides and recording archives.
    apple_logic_10_4_studio_horns => "Apple, Logic Pro 10.4 User Guide, Studio Horns overview, \
        https://help.apple.com/logicpro/mac/10.4/en.lproj/lgcp99c44c8f.html",
    apple_logic_10_4_studio_horns_keyswitches => "Apple, Logic Pro 10.4 User Guide, Studio Horns \
        keyswitch mapping, https://help.apple.com/logicpro/mac/10.4/en.lproj/lgcp882983f2.html",
    apple_logic_9_evp88 => "Apple, Logic Pro 9.1.6 help, Electric Piano Models Emulated by the \
        EVP88, \
        https://help.apple.com/logicpro/mac/9.1.6/en/logicpro/instruments/chapter_9_section_9.html",
    apple_logic_instruments => "Apple, Logic Pro Instruments for Mac (Logic Pro 12.3), \
        https://help.apple.com/pdf/logicpromac-instruments/en_US/logic-pro-mac-instruments-user-guide.pdf",
    apple_logic_ipad_studio_strings_keyswitches => "Apple, Logic Pro User Guide for iPad, Studio \
        Strings keyswitch mapping, \
        https://support.apple.com/guide/logicpro-ipad/lpipe5675ccd/ipados",
    apple_logic_studio_horns => "Apple, Logic Pro User Guide for Mac, Studio Horns in Logic Pro \
        for Mac, https://support.apple.com/guide/logicpro/studio-horns-overview-lgcpb0b2f2ab/mac",
    apple_logic_studio_horns_keyswitches => "Apple, Logic Pro User Guide for Mac, Studio Horns \
        keyswitch mapping, https://support.apple.com/en-ca/guide/logicpro/lgcp5a6d61ad/mac",
    apple_logic_vintage_b3 => "Apple, Logic Pro User Guide for Mac, Vintage B3 pages, \
        https://support.apple.com/guide/logicpro/percussion-effect-lgsia12dc4d/mac",
    arturia_clavinet_v_manual => "Arturia, Clavinet V User Manual, \
        https://r2.gear4music.com/media/94/943343/download_943343.pdf",
    arturia_stage_73_v2_manual => "Arturia, Stage-73 V2 User Manual (2021), \
        https://r2.gear4music.com/media/94/942952/download_942952.pdf",
    arturia_wurlitzer_v_manual => "Arturia, Wurlitzer V User's Manual 1.1.0, \
        https://downloads.arturia.com/products/wurlitzer-v/manual/WurlitzerV_Manual_1_1_0_EN.pdf",
    best_service_john_cage_prepared_piano => "Best Service, Big Fish Audio John Cage Prepared \
        Piano, https://www.bestservice.com/en/john_cage_prepared_piano.html",
    ilio_synchron_clarinet_eb => "ILIO, Vienna Symphonic Library Synchron Clarinet (Eb), \
        https://www.ilio.com/synchron-clarinet-eb",
    iowa_mis_alto_flute => "University of Iowa Electronic Music Studios, Musical Instrument \
        Samples: Alto Flute, https://theremin.music.uiowa.edu/MISaltoflute.html",
    jrhodes3d => "Jeff Learman, jRhodes3d, https://github.com/sfzinstruments/jlearman.jRhodes3d",
    moforte_geoshred_tenor_sax => "moForte, GeoShred help: Tenor Sax, \
        https://www.moforte.com/help/effects/tenorSax.html",
    native_instruments_session_horns_pro => "Native Instruments, Session Horns Pro, \
        articulations, \
        https://www.native-instruments.com/en/products/komplete/cinematic/session-horns-pro/articulations/",
    native_instruments_una_corda => "Native Instruments, Una Corda, \
        https://www.native-instruments.com/en/products/komplete/keys/una-corda/",
    orchestral_tools_berlin_brass => "Orchestral Tools, Berlin Brass - Notes (SINEplayer), \
        https://orchestraltools.helpscoutdocs.com/article/412-berlin-brass-notes",
    orchestral_tools_berlin_woodwinds => "Orchestral Tools, Berlin Woodwinds - Notes \
        (SINEplayer), \
        https://orchestraltools.helpscoutdocs.com/article/410-berlin-woodwinds-notes",
    orchestral_tools_berlin_woodwinds_additional => "Orchestral Tools, Berlin Woodwinds / \
        Additional Instruments - Notes, \
        https://orchestraltools.helpscoutdocs.com/article/353-berlin-woodwinds-additional-instruments-notes",
    samplemodeling_trumpet_3_manual => "Samplemodeling, The Trumpet 3 User Manual, Version 3 for \
        Kontakt 5, \
        https://www.samplemodeling.com/wp-content/uploads/2023/10/The-Trumpet-3-USER-MANUAL.pdf",
    sonatina_symphonic_orchestra => "Peter Eastman, Sonatina Symphonic Orchestra, \
        https://github.com/peastman/sso",
    soundiron_voices_of_rapture => "Soundiron, Voices of Rapture, \
        https://soundiron.com/collections/all-products/products/voices-of-rapture",
    spitfire_bbc_so_pro_manual => "Spitfire Audio, BBC Symphony Orchestra Professional user \
        manual v4, \
        https://www.spitfireaudio.com/cdn/shop/files/BBCSOPro_Manual_v4_96703953-4d7b-4d52-bdc8-a7b176ed342c.pdf",
    spitfire_harp_redux_manual => "Spitfire Audio, Skaila Kanga - Harp Redux user manual, \
        https://www.spitfireaudio.com/cdn/shop/files/SkailaKangaHarp_UserManual.pdf",
    spitfire_originals_cinematic_soft_piano => "Spitfire Audio, Originals Cinematic Soft Piano, \
        https://www.spitfireaudio.com/en-us/products/originals-cinematic-soft-piano",
    spitfire_originals_epic_choir => "Spitfire Audio, Originals Epic Choir, \
        https://www.spitfireaudio.com/originals-epic-choir",
    spitfire_originals_felt_piano => "Spitfire Audio, Originals Felt Piano, \
        https://www.spitfireaudio.com/originals-felt-piano",
    spitfire_studio_brass => "Spitfire Audio, Spitfire Studio Brass, \
        https://www.spitfireaudio.com/products/spitfire-studio-brass",
    spitfire_studio_brass_manual => "Spitfire Audio, Studio Brass user manual v1, \
        https://www.spitfireaudio.com/cdn/shop/files/SA_StudioBrass_UserManual_v1.pdf",
    spitfire_studio_brass_pro => "Spitfire Audio, Spitfire Studio Brass Professional, \
        https://www.spitfireaudio.com/spitfire-studio-brass-professional",
    spitfire_studio_woodwinds_pro => "Spitfire Audio, Spitfire Studio Woodwinds Professional, \
        https://www.spitfireaudio.com/spitfire-studio-woodwinds-professional",
    spitfire_symphony_orchestra_manual => "Spitfire Audio, Spitfire Symphony Orchestra User \
        Manual v5, \
        https://www.spitfireaudio.com/cdn/shop/files/Spitfire_Symphony_Orchestra_Manual_v5.pdf",
    vcsl => "Versilian Studios, Versilian Community Sample Library, \
        https://github.com/sgossner/VCSL",
    vsco2_ce => "Versilian Studios, VS Chamber Orchestra 2 Community Edition, SFZ branch, \
        https://github.com/sgossner/VSCO-2-CE/tree/SFZ",
    vsl_academy_bass_trombone => "Vienna Symphonic Library, Academy: Bass trombone, \
        https://www.vsl.co.at/academy/brass/bass-trombone",
    vsl_academy_bass_tuba => "Vienna Symphonic Library, Academy: Bass tuba, \
        https://www.vsl.co.at/academy/brass/bass-tuba",
    vsl_academy_bassoon => "Vienna Symphonic Library, Academy: Bassoon, \
        https://www.vsl.co.at/academy/woodwinds/bassoon",
    vsl_academy_cello => "Vienna Symphonic Library, Academy: Cello, \
        https://www.vsl.co.at/academy/strings/cello",
    vsl_academy_cimbasso => "Vienna Symphonic Library, Academy: Cimbasso, \
        https://www.vsl.co.at/academy/brass/cimbasso",
    vsl_academy_concert_flute => "Vienna Symphonic Library, Academy: Concert flute, \
        https://www.vsl.co.at/academy/woodwinds/concert-flute",
    vsl_academy_contrabass_tuba => "Vienna Symphonic Library, Academy: Contrabass tuba, \
        https://www.vsl.co.at/academy/brass/contrabass-tuba",
    vsl_academy_contrabassoon => "Vienna Symphonic Library, Academy: Contrabassoon, \
        https://www.vsl.co.at/academy/woodwinds/contrabassoon",
    vsl_academy_double_bass => "Vienna Symphonic Library, Academy: Double bass, \
        https://www.vsl.co.at/academy/strings/double-bass",
    vsl_academy_english_horn => "Vienna Symphonic Library, Academy: English horn, \
        https://www.vsl.co.at/academy/woodwinds/english-horn",
    vsl_academy_glockenspiel => "Vienna Symphonic Library, Academy: Glockenspiel, \
        https://www.vsl.co.at/academy/percussion/glockenspiel",
    vsl_academy_horn => "Vienna Symphonic Library, Academy: Viennese Horn, \
        https://www.vsl.co.at/academy/brass/horn-f",
    vsl_academy_marimba => "Vienna Symphonic Library, Academy: Marimba, \
        https://www.vsl.co.at/academy/percussion/marimba",
    vsl_academy_oboe => "Vienna Symphonic Library, Academy: Oboe, \
        https://www.vsl.co.at/academy/woodwinds/oboe",
    vsl_academy_piccolo => "Vienna Symphonic Library, Academy: Piccolo, \
        https://www.vsl.co.at/academy/woodwinds/piccolo",
    vsl_academy_piccolo_trumpet => "Vienna Symphonic Library, Academy: Piccolo trumpet, \
        https://www.vsl.co.at/academy/brass/piccolo-trumpet",
    vsl_academy_tenor_trombone => "Vienna Symphonic Library, Academy: Tenor trombone, \
        https://www.vsl.co.at/academy/brass/tenor-trombone",
    vsl_academy_timpani => "Vienna Symphonic Library, Academy: Timpani, \
        https://www.vsl.co.at/academy/percussion/timpani",
    vsl_academy_trumpet_c => "Vienna Symphonic Library, Academy: Trumpet in C, \
        https://www.vsl.co.at/academy/brass/trumpet-c",
    vsl_academy_tubular_bells => "Vienna Symphonic Library, Academy: Tubular Bells, \
        https://www.vsl.co.at/academy/percussion/tubular-bells",
    vsl_academy_vibraphone => "Vienna Symphonic Library, Academy: Vibraphone, \
        https://www.vsl.co.at/academy/percussion/vibraphone",
    vsl_academy_viola => "Vienna Symphonic Library, Academy: Viola, \
        https://www.vsl.co.at/academy/strings/viola",
    vsl_academy_violin => "Vienna Symphonic Library, Academy: Violin, \
        https://www.vsl.co.at/academy/strings/violin",
    vsl_academy_wagner_tuba => "Vienna Symphonic Library, Academy: Wagner tuba, \
        https://www.vsl.co.at/academy/brass/wagner-tuba",
    vsl_academy_xylophone => "Vienna Symphonic Library, Academy: Xylophone, \
        https://www.vsl.co.at/academy/percussion/xylophone",
    vsl_studio_alto_flute => "Vienna Symphonic Library, Studio Alto Flute, \
        https://www.vsl.co.at/products/studio/alto-flute",
    vsl_studio_bass_flute => "Vienna Symphonic Library, Studio Bass Flute, \
        https://www.vsl.co.at/products/studio/bass-flute",
    vsl_studio_cornet => "Vienna Symphonic Library, Studio Cornet, \
        https://www.vsl.co.at/products/studio/cornet",
    vsl_studio_flugelhorn => "Vienna Symphonic Library, Studio Flugelhorn, \
        https://www.vsl.co.at/products/studio/flugelhorn",
    vsl_studio_piccolo_trumpet => "Vienna Symphonic Library, Studio Piccolo Trumpet, \
        https://www.vsl.co.at/products/studio/piccolo-trumpet",
    vsl_studio_special_keyboards => "Vienna Symphonic Library, Studio Special Keyboards, \
        https://www.vsl.co.at/instruments/synchronized/special-keyboards",
    vsl_synchron_harp_player_manual => "Vienna Symphonic Library, Synchron Harp Player manual, \
        GUI overview, https://www.vsl.co.at/manuals/synchron-harp-player/gui-overview",
    vsl_synchron_oboe_d_amore => "Vienna Symphonic Library, Synchron Oboe d'Amore, \
        https://www.vsl.co.at/products/synchron/oboe-damore",
    vsl_synchron_percussion_1 => "Vienna Symphonic Library, Synchron Percussion I, \
        https://www.vsl.co.at/de/instruments/synchron/percussion-i",
    vsl_synchron_timpani_3 => "Vienna Symphonic Library, Synchron Timpani III, \
        https://www.vsl.co.at/products/synchron/timpani-3",
    vsl_synchron_wagner_tuba => "Vienna Symphonic Library, Synchron Wagner Tuba, \
        https://www.vsl.co.at/products/synchron/wagner-tuba",
    vsl_synchron_woodwinds => "Vienna Symphonic Library, Synchron Woodwinds, \
        https://www.vsl.co.at/products/synchron/woodwinds",

    // Reviews.
    epicomposer_hollywood_choirs => "Epicomposer, review of EastWest Hollywood Choirs (2018), \
        https://epicomposer.com/east-west-quantum-leap-hollywood-choirs-review/",
    musictech_hollywood_choirs => "MusicTech, review of EastWest Hollywood Choirs, \
        https://musictech.com/reviews/eastwest-hollywood-choirs-review/",
    sound_on_sound_eric_whitacre_choir => "Sound On Sound, review of Spitfire Audio Eric Whitacre \
        Choir, https://www.soundonsound.com/reviews/spitfire-audio-eric-whitacre-choir",
    sound_on_sound_scarbee_rsp_73 => "Sound On Sound, review of Scarbee RSP '73, \
        https://www.soundonsound.com/reviews/scarbee-rsp-73",
}

#[cfg(test)]
mod tests {
    use super::SOURCES;
    use std::collections::HashSet;

    /// Whether `text` names a URL (`http://` or `https://`) or a page (`p. 12`, `pp. 12-14`).
    fn names_url_or_page(text: &str) -> bool {
        text.contains("https://") || text.contains("http://") || names_page(text)
    }

    /// Whether `text` holds `p.` or `pp.` as a word, followed by a space and a digit.
    fn names_page(text: &str) -> bool {
        ["p. ", "pp. "].iter().any(|marker| {
            text.match_indices(marker).any(|(at, _)| {
                let starts_word = text[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|before| !before.is_alphanumeric());
                let digit_follows = text[at + marker.len()..]
                    .chars()
                    .next()
                    .is_some_and(|after| after.is_ascii_digit());
                starts_word && digit_follows
            })
        })
    }

    #[test]
    fn the_location_check_needs_a_url_or_a_page() {
        // TST-10: hand-written cases, accepted and rejected.
        assert!(names_url_or_page(
            "Wikipedia, Oboe, https://en.wikipedia.org/wiki/Oboe"
        ));
        assert!(names_url_or_page(
            "Rhodes Service Manual, http://www.fenderrhodes.com/"
        ));
        assert!(names_url_or_page(
            "A. Author, A Book, 4th edition (2016), p. 112"
        ));
        assert!(names_url_or_page("A. Author, A Book (2016), pp. 112-114"));
        assert!(!names_url_or_page("A. Author, A Book, 4th edition (2016)"));
        assert!(!names_url_or_page("A. Author, A Book, p. xii"));
        assert!(!names_url_or_page("A. Author, A Book, app. 3"));
        assert!(!names_url_or_page("A. Author, A Book, www.example.com"));
    }

    #[test]
    fn every_source_is_written_out_with_where_it_was_read() {
        assert_ne!(SOURCES, []);
        for (name, text) in SOURCES {
            assert!(!text.trim().is_empty(), "{name}");
            assert_eq!(text.trim(), *text, "{name}: no surrounding whitespace");
            assert!(!text.contains("  "), "{name}: no doubled space");
            // `cite!` appends ": " and "; ", which would read wrongly after a colon or semicolon.
            assert!(
                !text.ends_with([':', ';', ',', '.']),
                "{name}: ends in punctuation"
            );
            assert!(names_url_or_page(text), "{name}: names a URL or a page");
        }
    }

    #[test]
    fn names_and_texts_are_unique() {
        // A repeated name would add a second macro arm that never matches, so the compiler would
        // not report it.
        let mut names = HashSet::new();
        let mut texts = HashSet::new();
        for (name, text) in SOURCES {
            assert!(names.insert(name), "{name} is listed twice");
            assert!(
                texts.insert(text),
                "{name} repeats the text of another source"
            );
        }
    }

    #[test]
    fn source_expands_to_the_listed_text() {
        let listed = |wanted: &str| {
            SOURCES
                .iter()
                .find(|(name, _)| *name == wanted)
                .map(|(_, text)| *text)
        };
        assert_eq!(
            listed("wikipedia_piccolo"),
            Some(source!(wikipedia_piccolo))
        );
        assert_eq!(listed("forsyth"), Some(source!(forsyth)));
        assert_eq!(listed("vsco2_ce"), Some(source!(vsco2_ce)));
    }

    #[test]
    fn cite_writes_each_source_in_full_with_what_it_states() {
        // TST-10: the expected texts are written out by hand.
        assert_eq!(
            cite!(wikipedia_piccolo: "tessitura D5-C8"),
            "Wikipedia, Piccolo, https://en.wikipedia.org/wiki/Piccolo: tessitura D5-C8"
        );
        assert_eq!(
            cite!(wikipedia_oboe: "Bb3 to G6", wikipedia_cor_anglais: "E3 to C6",),
            "Wikipedia, Oboe, https://en.wikipedia.org/wiki/Oboe: Bb3 to G6; \
             Wikipedia, Cor anglais, https://en.wikipedia.org/wiki/Cor_anglais: E3 to C6"
        );
        assert_eq!(
            cite!(
                wikipedia_violin: "lowest G3",
                vsl_academy_violin: "range G3-C7",
                wikipedia_double_bass: "E1-G3"
            ),
            "Wikipedia, Violin, https://en.wikipedia.org/wiki/Violin: lowest G3; \
             Vienna Symphonic Library, Academy: Violin, \
             https://www.vsl.co.at/academy/strings/violin: range G3-C7; \
             Wikipedia, Double bass, https://en.wikipedia.org/wiki/Double_bass: E1-G3"
        );
    }
}
