use std::os::raw::c_void;

use crate::{ffi, script, Tag, Font, GlyphInfo, Mask, Script};
use crate::buffer::{Buffer, BufferScratchFlags};
use crate::ot::*;
use crate::unicode::{CharExt, GeneralCategory, GeneralCategoryExt, modified_combining_class};
use super::{hb_flag, hb_flag_unsafe};

const ARABIC_HAS_STCH: BufferScratchFlags = BufferScratchFlags::COMPLEX0;

const ARABIC_FEATURES: &[Tag] = &[
    feature::ISOLATED_FORMS,
    feature::TERMINAL_FORMS_1,
    feature::TERMINAL_FORMS_2,
    feature::TERMINAL_FORMS_3,
    feature::MEDIAL_FORMS_1,
    feature::MEDIAL_FORMS_2,
    feature::INITIAL_FORMS,
];

fn feature_is_syriac(tag: Tag) -> bool {
    matches!(tag.to_bytes()[3], b'2' | b'3')
}


#[allow(dead_code)]
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    ISOL = 0,
    FINA = 1,
    FIN2 = 2,
    FIN3 = 3,
    MEDI = 4,
    MED2 = 5,
    INIT = 6,
    NONE = 7,

    // We abuse the same byte for other things...
    StretchingFixed = 8,
    StretchingRepeating = 9,
}

impl Action {
    #[inline]
    fn is_stch(self) -> bool {
        matches!(self, Action::StretchingFixed | Action::StretchingRepeating)
    }
}


const STATE_TABLE: &[[(Action, Action, u16); 6]] = &[
    // jt_U,          jt_L,          jt_R,
    // jt_D,          jg_ALAPH,      jg_DALATH_RISH

    // State 0: prev was U, not willing to join.
    [
        (Action::NONE, Action::NONE, 0), (Action::NONE, Action::ISOL, 2), (Action::NONE, Action::ISOL, 1),
        (Action::NONE, Action::ISOL, 2), (Action::NONE, Action::ISOL, 1), (Action::NONE, Action::ISOL, 6),
    ],

    // State 1: prev was R or Action::ISOL/ALAPH, not willing to join.
    [
        (Action::NONE, Action::NONE, 0), (Action::NONE, Action::ISOL, 2), (Action::NONE, Action::ISOL, 1),
        (Action::NONE, Action::ISOL, 2), (Action::NONE, Action::FIN2, 5), (Action::NONE, Action::ISOL, 6),
    ],

    // State 2: prev was D/L in Action::ISOL form, willing to join.
    [
        (Action::NONE, Action::NONE, 0), (Action::NONE, Action::ISOL, 2), (Action::INIT, Action::FINA, 1),
        (Action::INIT, Action::FINA, 3), (Action::INIT, Action::FINA, 4), (Action::INIT, Action::FINA, 6),
    ],

    // State 3: prev was D in Action::FINA form, willing to join.
    [
        (Action::NONE, Action::NONE, 0), (Action::NONE, Action::ISOL, 2), (Action::MEDI, Action::FINA, 1),
        (Action::MEDI, Action::FINA, 3), (Action::MEDI, Action::FINA, 4), (Action::MEDI, Action::FINA, 6),
    ],

    // State 4: prev was Action::FINA ALAPH, not willing to join.
    [
        (Action::NONE, Action::NONE, 0), (Action::NONE, Action::ISOL, 2), (Action::MED2, Action::ISOL, 1),
        (Action::MED2, Action::ISOL, 2), (Action::MED2, Action::FIN2, 5), (Action::MED2, Action::ISOL, 6),
    ],

    // State 5: prev was FIN2/FIN3 ALAPH, not willing to join.
    [
        (Action::NONE, Action::NONE, 0), (Action::NONE, Action::ISOL, 2), (Action::ISOL, Action::ISOL, 1),
        (Action::ISOL, Action::ISOL, 2), (Action::ISOL, Action::FIN2, 5), (Action::ISOL, Action::ISOL, 6),
    ],

    // State 6: prev was DALATH/RISH, not willing to join.
    [
        (Action::NONE, Action::NONE, 0), (Action::NONE, Action::ISOL, 2), (Action::NONE, Action::ISOL, 1),
        (Action::NONE, Action::ISOL, 2), (Action::NONE, Action::FIN3, 5), (Action::NONE, Action::ISOL, 6),
    ]
];


#[derive(Clone, Copy, PartialEq, Debug)]
pub enum JoiningType {
    U = 0,
    L = 1,
    R = 2,
    D = 3,
    // We don't have C, like harfbuzz, because Rust doesn't allow duplicated enum variants.
    GroupAlaph = 4,
    GroupDalathRish = 5,
    T = 7,
    X = 8, // means: use general-category to choose between U or T.
}


impl GlyphInfo {
    fn arabic_shaping_action(&self) -> Action {
        unsafe {
            let v: &ffi::hb_var_int_t = std::mem::transmute(&self.var2);
            std::mem::transmute(v.var_u8[2])
        }
    }

    fn set_arabic_shaping_action(&mut self, action: Action) {
        unsafe {
            let v: &mut ffi::hb_var_int_t = std::mem::transmute(&mut self.var2);
            v.var_u8[2] = action as u8;
        }
    }
}


pub struct ArabicShapePlan {
    // The "+ 1" in the next array is to accommodate for the "NONE" command,
    // which is not an OpenType feature, but this simplifies the code by not
    // having to do a "if (... < NONE) ..." and just rely on the fact that
    // mask_array[NONE] == 0.
    mask_array: [Mask; ARABIC_FEATURES.len() + 1],

    has_stch: bool,
}

impl ArabicShapePlan {
    fn from_ptr(plan: *const c_void) -> &'static ArabicShapePlan {
        unsafe { &*(plan as *const ArabicShapePlan) }
    }
}


#[no_mangle]
pub extern "C" fn hb_ot_complex_collect_features_arabic(planner: *mut ffi::hb_ot_shape_planner_t) {
    let mut planner = ShapePlanner::from_ptr_mut(planner);
    collect_features(&mut planner)
}

fn collect_features(planner: &mut ShapePlanner) {
    // We apply features according to the Arabic spec, with pauses
    // in between most.
    //
    // The pause between init/medi/... and rlig is required.  See eg:
    // https://bugzilla.mozilla.org/show_bug.cgi?id=644184
    //
    // The pauses between init/medi/... themselves are not necessarily
    // needed as only one of those features is applied to any character.
    // The only difference it makes is when fonts have contextual
    // substitutions.  We now follow the order of the spec, which makes
    // for better experience if that's what Uniscribe is doing.
    //
    // At least for Arabic, looks like Uniscribe has a pause between
    // rlig and calt.  Otherwise the IranNastaliq's ALLAH ligature won't
    // work.  However, testing shows that rlig and calt are applied
    // together for Mongolian in Uniscribe.  As such, we only add a
    // pause for Arabic, not other scripts.
    //
    // A pause after calt is required to make KFGQPC Uthmanic Script HAFS
    // work correctly.  See https://github.com/harfbuzz/harfbuzz/issues/505

    planner.ot_map.enable_feature(feature::STRETCHING_GLYPH_DECOMPOSITION, FeatureFlags::NONE, 1);
    planner.ot_map.add_gsub_pause(Some(record_stch_raw));

    planner.ot_map.enable_feature(feature::GLYPH_COMPOSITION_DECOMPOSITION, FeatureFlags::NONE, 1);
    planner.ot_map.enable_feature(feature::LOCALIZED_FORMS, FeatureFlags::NONE, 1);

    planner.ot_map.add_gsub_pause(None);

    for feature in ARABIC_FEATURES {
        let has_fallback = planner.script() == script::ARABIC && !feature_is_syriac(*feature);
        let flags = if has_fallback { FeatureFlags::HAS_FALLBACK } else { FeatureFlags::NONE };
        planner.ot_map.add_feature(*feature, flags, 1);
        planner.ot_map.add_gsub_pause(None);
    }

    // Normally, Unicode says a ZWNJ means "don't ligate".  In Arabic script
    // however, it says a ZWJ should also mean "don't ligate".  So we run
    // the main ligating features as MANUAL_ZWJ.

    planner.ot_map.enable_feature(feature::REQUIRED_LIGATURES,
                                  FeatureFlags::MANUAL_ZWJ | FeatureFlags::HAS_FALLBACK, 1);

    if planner.script() == script::ARABIC {
        planner.ot_map.add_gsub_pause(Some(fallback_shape_raw));
    }

    // No pause after rclt.
    // See 98460779bae19e4d64d29461ff154b3527bf8420
    planner.ot_map.enable_feature(feature::REQUIRED_CONTEXTUAL_ALTERNATES, FeatureFlags::MANUAL_ZWJ, 1);
    planner.ot_map.enable_feature(feature::CONTEXTUAL_ALTERNATES, FeatureFlags::MANUAL_ZWJ, 1);
    planner.ot_map.add_gsub_pause(None);

    // And undo here.

    // The spec includes 'cswh'.  Earlier versions of Windows
    // used to enable this by default, but testing suggests
    // that Windows 8 and later do not enable it by default,
    // and spec now says 'Off by default'.
    // We disabled this in ae23c24c32.
    // Note that IranNastaliq uses this feature extensively
    // to fixup broken glyph sequences.  Oh well...
    // Test case: U+0643,U+0640,U+0631.

    // planner.ot_map.enable_feature(feature::CONTEXTUAL_SWASH, FeatureFlags::NONE, 1);
    planner.ot_map.enable_feature(feature::MARK_POSITIONING_VIA_SUBSTITUTION, FeatureFlags::NONE, 1);
}

extern "C" fn fallback_shape_raw(
    _: *const ffi::hb_ot_shape_plan_t,
    _: *mut ffi::hb_font_t,
    _: *mut ffi::hb_buffer_t,
) {
}

// Stretch feature: "stch".
// See example here:
// https://docs.microsoft.com/en-us/typography/script-development/syriac
// We implement this in a generic way, such that the Arabic subtending
// marks can use it as well.
extern "C" fn record_stch_raw(
    plan: *const ffi::hb_ot_shape_plan_t,
    font: *mut ffi::hb_font_t,
    buffer: *mut ffi::hb_buffer_t,
) {
    let plan = ShapePlan::from_ptr(plan);
    let font = Font::from_ptr(font);
    let mut buffer = Buffer::from_ptr_mut(buffer);
    record_stch(&plan, font, &mut buffer);
}

fn record_stch(plan: &ShapePlan, _: &Font, buffer: &mut Buffer) {
    let arabic_plan = ArabicShapePlan::from_ptr(plan.data() as _);
    if !arabic_plan.has_stch {
        return;
    }

    // 'stch' feature was just applied.  Look for anything that multiplied,
    // and record it for stch treatment later.  Note that rtlm, frac, etc
    // are applied before stch, but we assume that they didn't result in
    // anything multiplying into 5 pieces, so it's safe-ish...

    let len = buffer.len();
    let info = buffer.info_mut();
    let mut has_stch = false;
    for i in 0..len {
        if info[i].is_multiplied() {
            let comp = if info[i].lig_comp() % 2 != 0 {
                Action::StretchingRepeating
            } else {
                Action::StretchingFixed
            };

            info[i].set_arabic_shaping_action(comp);
            has_stch = true;
        }
    }

    if has_stch {
        buffer.set_scratch_flags(buffer.scratch_flags() | ARABIC_HAS_STCH);
    }
}

#[no_mangle]
pub extern "C" fn hb_ot_complex_postprocess_glyphs_arabic(
    plan: *const ffi::hb_ot_shape_plan_t,
    buffer: *mut ffi::hb_buffer_t,
    font: *mut ffi::hb_font_t,
) {
    let plan = ShapePlan::from_ptr(plan);
    let mut buffer = Buffer::from_ptr_mut(buffer);
    let font = Font::from_ptr(font);
    postprocess_glyphs(&plan, &font, &mut buffer)
}

fn postprocess_glyphs(_: &ShapePlan, font: &Font, buffer: &mut Buffer) {
    apply_stch(font, buffer)
}

fn apply_stch(font: &Font, buffer: &mut Buffer) {
    if !buffer.scratch_flags().contains(ARABIC_HAS_STCH) {
        return;
    }

    // The Arabic shaper currently always processes in RTL mode, so we should
    // stretch / position the stretched pieces to the left / preceding glyphs.

    // We do a two pass implementation:
    // First pass calculates the exact number of extra glyphs we need,
    // We then enlarge buffer to have that much room,
    // Second pass applies the stretch, copying things to the end of buffer.

    let mut extra_glyphs_needed: usize = 0; // Set during MEASURE, used during CUT
    const MEASURE: usize = 0;
    const CUT: usize = 1;

    for step in 0..2 {
        let new_len = buffer.len() + extra_glyphs_needed; // write head during CUT
        let mut i = buffer.len();
        let mut j = new_len;
        while i != 0 {
            if !buffer.info()[i - 1].arabic_shaping_action().is_stch() {
                if step == CUT {
                    j -= 1;
                    buffer.info_mut()[j] = buffer.info()[i - 1];
                    buffer.pos_mut()[j] = buffer.pos()[i - 1];
                }

                i -= 1;
                continue;
            }

            // Yay, justification!

            let mut w_total = 0;     // Total to be filled
            let mut w_fixed = 0;     // Sum of fixed tiles
            let mut w_repeating = 0; // Sum of repeating tiles
            let mut n_repeating: i32 = 0;

            let end = i;
            while i != 0 && buffer.info()[i - 1].arabic_shaping_action().is_stch() {
                i -= 1;
                let width = font.glyph_h_advance(buffer.info()[i].codepoint) as i32;

                if buffer.info()[i].arabic_shaping_action() == Action::StretchingFixed {
                    w_fixed += width;
                } else {
                    w_repeating += width;
                    n_repeating += 1;
                }
            }

            let start = i;
            let mut context = i;
            while context != 0 &&
                !buffer.info()[context - 1].arabic_shaping_action().is_stch() &&
                (buffer.info()[context - 1].is_default_ignorable() ||
                    is_word_category(buffer.info()[context - 1].general_category()))
            {
                context -= 1;
                w_total += buffer.pos()[context].x_advance;
            }

            i += 1; // Don't touch i again.

            // Number of additional times to repeat each repeating tile.
            let mut n_copies: i32 = 0;

            let w_remaining = w_total - w_fixed;
            if w_remaining > w_repeating && w_repeating > 0 {
                n_copies = w_remaining / (w_repeating) - 1;
            }

            // See if we can improve the fit by adding an extra repeat and squeezing them together a bit.
            let mut extra_repeat_overlap = 0;
            let shortfall = w_remaining - w_repeating * (n_copies + 1);
            if shortfall > 0 && n_repeating > 0 {
                n_copies += 1;
                let excess = (n_copies + 1) * w_repeating - w_remaining;
                if excess > 0 {
                    extra_repeat_overlap = excess / (n_copies * n_repeating);
                }
            }

            if step == MEASURE {
                extra_glyphs_needed += (n_copies * n_repeating) as usize;
            } else {
                buffer.unsafe_to_break(context, end);
                let mut x_offset = 0;
                for k in (start+1..=end).rev() {
                    let width = font.glyph_h_advance(buffer.info()[k - 1].codepoint) as i32;

                    let mut repeat = 1;
                    if buffer.info()[k - 1].arabic_shaping_action() == Action::StretchingRepeating {
                        repeat += n_copies;
                    }

                    for n in 0..repeat {
                        x_offset -= width;
                        if n > 0 {
                            x_offset += extra_repeat_overlap;
                        }

                        buffer.pos_mut()[k - 1].x_offset = x_offset;

                        // Append copy.
                        j -= 1;
                        buffer.info_mut()[j] = buffer.info()[k - 1];
                        buffer.pos_mut()[j] = buffer.pos()[k - 1];
                    }
                }
            }

            i -= 1;
        }

        if step == MEASURE {
            buffer.ensure(buffer.len() + extra_glyphs_needed);
        } else {
            debug_assert_eq!(j, 0);
            buffer.set_len(new_len);
        }
    }
}

// See:
// https://github.com/harfbuzz/harfbuzz/commit/6e6f82b6f3dde0fc6c3c7d991d9ec6cfff57823d#commitcomment-14248516
fn is_word_category(gc: GeneralCategory) -> bool {
    (hb_flag_unsafe(gc.to_hb()) &
        (   hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_UNASSIGNED) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_PRIVATE_USE) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_MODIFIER_LETTER) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_OTHER_LETTER) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_SPACING_MARK) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_ENCLOSING_MARK) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_NON_SPACING_MARK) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_DECIMAL_NUMBER) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_LETTER_NUMBER) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_OTHER_NUMBER) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_CURRENCY_SYMBOL) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_MODIFIER_SYMBOL) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_MATH_SYMBOL) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_OTHER_SYMBOL)
        )) != 0
}

#[no_mangle]
pub extern "C" fn hb_ot_complex_setup_masks_arabic(
    plan: *const ffi::hb_ot_shape_plan_t,
    buffer: *mut ffi::hb_buffer_t,
    _: *mut ffi::hb_font_t,
) {
    let plan = ShapePlan::from_ptr(plan);
    let arabic_plan = ArabicShapePlan::from_ptr(plan.data() as _);
    let mut buffer = Buffer::from_ptr_mut(buffer);
    setup_masks(arabic_plan, plan.script(), &mut buffer)
}

#[no_mangle]
pub extern "C" fn hb_ot_complex_setup_masks_arabic_plan(
    arabic_plan: *const c_void,
    buffer: *mut ffi::hb_buffer_t,
    script: ffi::hb_script_t,
) {
    let arabic_plan = ArabicShapePlan::from_ptr(arabic_plan);
    let mut buffer = Buffer::from_ptr_mut(buffer);
    setup_masks(arabic_plan, Script::from_raw(script), &mut buffer)
}

pub(crate) fn setup_masks(plan: &ArabicShapePlan, script: Script, buffer: &mut Buffer) {
    arabic_joining(buffer);
    if script == script::MONGOLIAN {
        mongolian_variation_selectors(buffer);
    }

    let len = buffer.len();
    let info = buffer.info_mut();
    for i in 0..len {
        info[i].mask |= plan.mask_array[info[i].arabic_shaping_action() as usize];
    }
}

fn arabic_joining(buffer: &mut Buffer) {
    let mut prev: Option<usize> = None;
    let mut state = 0;

    // Check pre-context.
    for i in 0..buffer.context_len(0) {
        let c = buffer.context(0, i);
        let this_type = get_joining_type(c, c.general_category());
        if this_type == JoiningType::T {
            continue;
        }

        state = STATE_TABLE[state][this_type as usize].2 as usize;
        break;
    }

    for i in 0..buffer.len() {
        let this_type = get_joining_type(
            buffer.info()[i].as_char(),
            buffer.info()[i].general_category(),
        );
        if this_type == JoiningType::T {
            buffer.info_mut()[i].set_arabic_shaping_action(Action::NONE);
            continue;
        }

        let entry = &STATE_TABLE[state][this_type as usize];
        if entry.0 != Action::NONE && prev.is_some() {
            if let Some(prev) = prev {
                buffer.info_mut()[prev].set_arabic_shaping_action(entry.0);
                buffer.unsafe_to_break(prev, i + 1);
            }
        }

        buffer.info_mut()[i].set_arabic_shaping_action(entry.1);

        prev = Some(i);
        state = entry.2 as usize;
    }

    for i in 0..buffer.context_len(1) {
        let c = buffer.context(1, i);
        let this_type = get_joining_type(c, c.general_category());
        if this_type == JoiningType::T {
            continue;
        }

        let entry = &STATE_TABLE[state][this_type as usize];
        if entry.0 != Action::NONE && prev.is_some() {
            if let Some(prev) = prev {
                buffer.info_mut()[prev].set_arabic_shaping_action(entry.0);
            }
        }

        break;
    }
}

fn mongolian_variation_selectors(buffer: &mut Buffer) {
   // Copy arabic_shaping_action() from base to Mongolian variation selectors.
    let len = buffer.len();
    let info = buffer.info_mut();
    for i in 1..len {
        if (0x180B..=0x180D).contains(&info[i].codepoint) {
            info[i].set_arabic_shaping_action(info[i - 1].arabic_shaping_action());
        }
    }
}

#[no_mangle]
pub extern "C" fn hb_ot_complex_data_create_arabic(
    plan: *const ffi::hb_ot_shape_plan_t,
) -> *mut c_void {
    let plan = ShapePlan::from_ptr(plan);
    Box::into_raw(Box::new(data_create(&plan))) as _
}

pub fn data_create(plan: &ShapePlan) -> ArabicShapePlan {
    let mut arabic_plan = ArabicShapePlan {
        mask_array: [0; ARABIC_FEATURES.len() + 1],
        has_stch: false,
    };

    arabic_plan.has_stch = plan.ot_map.get_1_mask(feature::STRETCHING_GLYPH_DECOMPOSITION) != 0;
    for i in 0..ARABIC_FEATURES.len() {
        arabic_plan.mask_array[i] = plan.ot_map.get_1_mask(ARABIC_FEATURES[i]);
    }

    arabic_plan
}

#[no_mangle]
pub extern "C" fn hb_ot_complex_data_destroy_arabic(data: *mut c_void) {
    unsafe { Box::from_raw(data) };
}

fn get_joining_type(u: char, gc: GeneralCategory) -> JoiningType {
    let j_type = super::arabic_table::joining_type(u);
    if j_type != JoiningType::X {
        return j_type;
    }

    let ok = hb_flag_unsafe(gc.to_hb()) &
        (hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_NON_SPACING_MARK) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_ENCLOSING_MARK) |
            hb_flag(ffi::HB_UNICODE_GENERAL_CATEGORY_FORMAT));

    if ok != 0 { JoiningType::T } else { JoiningType::U }
}

#[no_mangle]
pub extern "C" fn hb_ot_complex_reorder_marks_arabic(
    _: *const ffi::hb_ot_shape_plan_t,
    buffer: *mut ffi::hb_buffer_t,
    start: u32,
    end: u32,
) {
    let mut buffer = Buffer::from_ptr_mut(buffer);
    reorder_marks(start as usize, end as usize, &mut buffer)
}

// http://www.unicode.org/reports/tr53/
const MODIFIER_COMBINING_MARKS: &[u32] = &[
    0x0654, // ARABIC HAMZA ABOVE
    0x0655, // ARABIC HAMZA BELOW
    0x0658, // ARABIC MARK NOON GHUNNA
    0x06DC, // ARABIC SMALL HIGH SEEN
    0x06E3, // ARABIC SMALL LOW SEEN
    0x06E7, // ARABIC SMALL HIGH YEH
    0x06E8, // ARABIC SMALL HIGH NOON
    0x08D3, // ARABIC SMALL LOW WAW
    0x08F3, // ARABIC SMALL HIGH WAW
];

fn reorder_marks(mut start: usize, end: usize, buffer: &mut Buffer) {
    const MAX_COMBINING_MARKS: usize = 32;

    let mut i = start;
    for cc in [220u8, 230].iter().cloned() {
        while i < end && buffer.info()[i].modified_combining_class() < cc {
            i += 1;
        }

        if i == end {
            break;
        }

        if buffer.info()[i].modified_combining_class() > cc {
            continue;
        }

        let mut j = i;
        while j < end &&
            buffer.info()[j].modified_combining_class() == cc &&
            MODIFIER_COMBINING_MARKS.contains(&buffer.info()[j].codepoint)
        {
            j += 1;
        }

        if i == j {
            continue;
        }

        // Shift it!
        let mut temp = [GlyphInfo::default(); MAX_COMBINING_MARKS];
        debug_assert!(j - i <= MAX_COMBINING_MARKS);
        buffer.merge_clusters(start, j);

        for k in 0..j-i {
            temp[k] = buffer.info()[k + i];
        }

        for k in (0..i-start).rev() {
            buffer.info_mut()[k + start + j - i] = buffer.info()[k + start];
        }

        for k in 0..j-i {
            buffer.info_mut()[k + start] = temp[k];
        }

        // Renumber CC such that the reordered sequence is still sorted.
        // 22 and 26 are chosen because they are smaller than all Arabic categories,
        // and are folded back to 220/230 respectively during fallback mark positioning.
        //
        // We do this because the CGJ-handling logic in the normalizer relies on
        // mark sequences having an increasing order even after this reordering.
        // https://github.com/harfbuzz/harfbuzz/issues/554
        // This, however, does break some obscure sequences, where the normalizer
        // might compose a sequence that it should not.  For example, in the seequence
        // ALEF, HAMZAH, MADDAH, we should NOT try to compose ALEF+MADDAH, but with this
        // renumbering, we will.
        let new_start = start + j - i;
        let new_cc = if cc == 220 {
            modified_combining_class::CCC22
        } else {
            modified_combining_class::CCC26
        };

        while start < new_start {
            buffer.info_mut()[start].set_modified_combining_class(new_cc);
            start += 1;
        }

        i = j;
    }
}
