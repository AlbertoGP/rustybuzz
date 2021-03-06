/*
 * Copyright © 1998-2004  David Turner and Werner Lemberg
 * Copyright © 2004,2007,2009  Red Hat, Inc.
 * Copyright © 2011,2012  Google, Inc.
 *
 *  This is part of HarfBuzz, a text shaping library.
 *
 * Permission is hereby granted, without written agreement and without
 * license or royalty fees, to use, copy, modify, and distribute this
 * software and its documentation for any purpose, provided that the
 * above copyright notice and the following two paragraphs appear in
 * all copies of this software.
 *
 * IN NO EVENT SHALL THE COPYRIGHT HOLDER BE LIABLE TO ANY PARTY FOR
 * DIRECT, INDIRECT, SPECIAL, INCIDENTAL, OR CONSEQUENTIAL DAMAGES
 * ARISING OUT OF THE USE OF THIS SOFTWARE AND ITS DOCUMENTATION, EVEN
 * IF THE COPYRIGHT HOLDER HAS BEEN ADVISED OF THE POSSIBILITY OF SUCH
 * DAMAGE.
 *
 * THE COPYRIGHT HOLDER SPECIFICALLY DISCLAIMS ANY WARRANTIES, INCLUDING,
 * BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
 * FITNESS FOR A PARTICULAR PURPOSE.  THE SOFTWARE PROVIDED HEREUNDER IS
 * ON AN "AS IS" BASIS, AND THE COPYRIGHT HOLDER HAS NO OBLIGATION TO
 * PROVIDE MAINTENANCE, SUPPORT, UPDATES, ENHANCEMENTS, OR MODIFICATIONS.
 *
 * Red Hat Author(s): Owen Taylor, Behdad Esfahbod
 * Google Author(s): Behdad Esfahbod
 */

#ifndef HB_H_IN
#error "Include <hb.h> instead."
#endif

#ifndef HB_BUFFER_H
#define HB_BUFFER_H

#include "hb-common.h"
#include "hb-unicode.h"
#include "hb-font.h"

HB_BEGIN_DECLS

/**
 * hb_glyph_info_t:
 * @codepoint: either a Unicode code point (before shaping) or a glyph index
 *             (after shaping).
 * @cluster: the index of the character in the original text that corresponds
 *           to this #hb_glyph_info_t, or whatever the client passes to
 *           hb_buffer_add(). More than one #hb_glyph_info_t can have the same
 *           @cluster value, if they resulted from the same character (e.g. one
 *           to many glyph substitution), and when more than one character gets
 *           merged in the same glyph (e.g. many to one glyph substitution) the
 *           #hb_glyph_info_t will have the smallest cluster value of them.
 *           By default some characters are merged into the same cluster
 *           (e.g. combining marks have the same cluster as their bases)
 *           even if they are separate glyphs, hb_buffer_set_cluster_level()
 *           allow selecting more fine-grained cluster handling.
 *
 * The #hb_glyph_info_t is the structure that holds information about the
 * glyphs and their relation to input text.
 */
typedef struct hb_glyph_info_t
{
    hb_codepoint_t codepoint;
    /*< private >*/
    hb_mask_t mask;
    /*< public >*/
    uint32_t cluster;

    /*< private >*/
    hb_var_int_t var1;
    hb_var_int_t var2;
} hb_glyph_info_t;

/**
 * hb_glyph_flags_t:
 * @HB_GLYPH_FLAG_UNSAFE_TO_BREAK: Indicates that if input text is broken at the
 * 				   beginning of the cluster this glyph is part of,
 * 				   then both sides need to be re-shaped, as the
 * 				   result might be different.  On the flip side,
 * 				   it means that when this flag is not present,
 * 				   then it's safe to break the glyph-run at the
 * 				   beginning of this cluster, and the two sides
 * 				   represent the exact same result one would get
 * 				   if breaking input text at the beginning of
 * 				   this cluster and shaping the two sides
 * 				   separately.  This can be used to optimize
 * 				   paragraph layout, by avoiding re-shaping
 * 				   of each line after line-breaking, or limiting
 * 				   the reshaping to a small piece around the
 * 				   breaking point only.
 * @HB_GLYPH_FLAG_DEFINED: All the currently defined flags.
 *
 * Since: 1.5.0
 */
typedef enum { /*< flags >*/
               HB_GLYPH_FLAG_UNSAFE_TO_BREAK = 0x00000001,

               HB_GLYPH_FLAG_DEFINED = 0x00000001 /* OR of all defined flags */
} hb_glyph_flags_t;

HB_EXTERN hb_glyph_flags_t hb_glyph_info_get_glyph_flags(const hb_glyph_info_t *info);

#define hb_glyph_info_get_glyph_flags(info) ((hb_glyph_flags_t)((unsigned int)(info)->mask & HB_GLYPH_FLAG_DEFINED))

/**
 * hb_glyph_position_t:
 * @x_advance: how much the line advances after drawing this glyph when setting
 *             text in horizontal direction.
 * @y_advance: how much the line advances after drawing this glyph when setting
 *             text in vertical direction.
 * @x_offset: how much the glyph moves on the X-axis before drawing it, this
 *            should not affect how much the line advances.
 * @y_offset: how much the glyph moves on the Y-axis before drawing it, this
 *            should not affect how much the line advances.
 *
 * The #hb_glyph_position_t is the structure that holds the positions of the
 * glyph in both horizontal and vertical directions. All positions in
 * #hb_glyph_position_t are relative to the current point.
 *
 */
typedef struct hb_glyph_position_t
{
    hb_position_t x_advance;
    hb_position_t y_advance;
    hb_position_t x_offset;
    hb_position_t y_offset;

    /*< private >*/
    hb_var_int_t var;
} hb_glyph_position_t;

/**
 * hb_segment_properties_t:
 * @direction: the #hb_direction_t of the buffer, see hb_buffer_set_direction().
 * @script: the #hb_script_t of the buffer, see hb_buffer_set_script().
 * @language: the #hb_language_t of the buffer, see hb_buffer_set_language().
 *
 * The structure that holds various text properties of an #hb_buffer_t. Can be
 * set and retrieved using hb_buffer_set_segment_properties() and
 * hb_buffer_get_segment_properties(), respectively.
 */
typedef struct hb_segment_properties_t
{
    hb_direction_t direction;
    hb_script_t script;
    const char *language;
    /*< private >*/
    void *reserved1;
    void *reserved2;
} hb_segment_properties_t;

#define HB_SEGMENT_PROPERTIES_DEFAULT                                                                                  \
    {                                                                                                                  \
        HB_DIRECTION_INVALID, HB_SCRIPT_INVALID, NULL, (void *)0, (void *)0                                            \
    }

HB_EXTERN hb_bool_t hb_segment_properties_equal(const hb_segment_properties_t *a, const hb_segment_properties_t *b);

HB_EXTERN unsigned int hb_segment_properties_hash(const hb_segment_properties_t *p);

/**
 * hb_buffer_t:
 *
 * The main structure holding the input text and its properties before shaping,
 * and output glyphs and their information after shaping.
 */

typedef struct hb_buffer_t hb_buffer_t;

HB_EXTERN hb_buffer_t *hb_buffer_create(void);

HB_EXTERN hb_buffer_t *hb_buffer_get_empty(void);

HB_EXTERN hb_buffer_t *hb_buffer_reference(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_destroy(hb_buffer_t *buffer);

/**
 * hb_buffer_content_type_t:
 * @HB_BUFFER_CONTENT_TYPE_INVALID: Initial value for new buffer.
 * @HB_BUFFER_CONTENT_TYPE_UNICODE: The buffer contains input characters (before shaping).
 * @HB_BUFFER_CONTENT_TYPE_GLYPHS: The buffer contains output glyphs (after shaping).
 */
typedef enum {
    HB_BUFFER_CONTENT_TYPE_INVALID = 0,
    HB_BUFFER_CONTENT_TYPE_UNICODE,
    HB_BUFFER_CONTENT_TYPE_GLYPHS
} hb_buffer_content_type_t;

HB_EXTERN void hb_buffer_set_content_type(hb_buffer_t *buffer, hb_buffer_content_type_t content_type);

HB_EXTERN hb_buffer_content_type_t hb_buffer_get_content_type(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_set_direction(hb_buffer_t *buffer, hb_direction_t direction);

HB_EXTERN hb_direction_t hb_buffer_get_direction(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_set_script(hb_buffer_t *buffer, hb_script_t script);

HB_EXTERN hb_script_t hb_buffer_get_script(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_set_language(hb_buffer_t *buffer, const char *language);

HB_EXTERN const char *hb_buffer_get_language(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_set_segment_properties(hb_buffer_t *buffer, const hb_segment_properties_t *props);

HB_EXTERN void hb_buffer_get_segment_properties(hb_buffer_t *buffer, hb_segment_properties_t *props);

HB_EXTERN void hb_buffer_guess_segment_properties(hb_buffer_t *buffer);

/**
 * hb_buffer_flags_t:
 * @HB_BUFFER_FLAG_DEFAULT: the default buffer flag.
 * @HB_BUFFER_FLAG_BOT: flag indicating that special handling of the beginning
 *                      of text paragraph can be applied to this buffer. Should usually
 *                      be set, unless you are passing to the buffer only part
 *                      of the text without the full context.
 * @HB_BUFFER_FLAG_EOT: flag indicating that special handling of the end of text
 *                      paragraph can be applied to this buffer, similar to
 *                      @HB_BUFFER_FLAG_BOT.
 * @HB_BUFFER_FLAG_PRESERVE_DEFAULT_IGNORABLES:
 *                      flag indication that character with Default_Ignorable
 *                      Unicode property should use the corresponding glyph
 *                      from the font, instead of hiding them (done by
 *                      replacing them with the space glyph and zeroing the
 *                      advance width.)  This flag takes precedence over
 *                      @HB_BUFFER_FLAG_REMOVE_DEFAULT_IGNORABLES.
 * @HB_BUFFER_FLAG_REMOVE_DEFAULT_IGNORABLES:
 *                      flag indication that character with Default_Ignorable
 *                      Unicode property should be removed from glyph string
 *                      instead of hiding them (done by replacing them with the
 *                      space glyph and zeroing the advance width.)
 *                      @HB_BUFFER_FLAG_PRESERVE_DEFAULT_IGNORABLES takes
 *                      precedence over this flag. Since: 1.8.0
 * @HB_BUFFER_FLAG_DO_NOT_INSERT_DOTTED_CIRCLE:
 *                      flag indicating that a dotted circle should
 *                      not be inserted in the rendering of incorrect
 *                      character sequences (such at <0905 093E>). Since: 2.4
 *
 * Since: 0.9.20
 */
typedef enum { /*< flags >*/
               HB_BUFFER_FLAG_DEFAULT = 0x00000000u,
               HB_BUFFER_FLAG_BOT = 0x00000001u, /* Beginning-of-text */
               HB_BUFFER_FLAG_EOT = 0x00000002u, /* End-of-text */
               HB_BUFFER_FLAG_PRESERVE_DEFAULT_IGNORABLES = 0x00000004u,
               HB_BUFFER_FLAG_REMOVE_DEFAULT_IGNORABLES = 0x00000008u,
               HB_BUFFER_FLAG_DO_NOT_INSERT_DOTTED_CIRCLE = 0x00000010u
} hb_buffer_flags_t;

HB_EXTERN void hb_buffer_set_flags(hb_buffer_t *buffer, hb_buffer_flags_t flags);

HB_EXTERN hb_buffer_flags_t hb_buffer_get_flags(hb_buffer_t *buffer);

/**
 * hb_buffer_cluster_level_t:
 * @HB_BUFFER_CLUSTER_LEVEL_MONOTONE_GRAPHEMES: Return cluster values grouped by graphemes into
 *   monotone order.
 * @HB_BUFFER_CLUSTER_LEVEL_MONOTONE_CHARACTERS: Return cluster values grouped into monotone order.
 * @HB_BUFFER_CLUSTER_LEVEL_CHARACTERS: Don't group cluster values.
 * @HB_BUFFER_CLUSTER_LEVEL_DEFAULT: Default cluster level,
 *   equal to @HB_BUFFER_CLUSTER_LEVEL_MONOTONE_GRAPHEMES.
 *
 * Since: 0.9.42
 */
typedef enum {
    HB_BUFFER_CLUSTER_LEVEL_MONOTONE_GRAPHEMES = 0,
    HB_BUFFER_CLUSTER_LEVEL_MONOTONE_CHARACTERS = 1,
    HB_BUFFER_CLUSTER_LEVEL_CHARACTERS = 2,
    HB_BUFFER_CLUSTER_LEVEL_DEFAULT = HB_BUFFER_CLUSTER_LEVEL_MONOTONE_GRAPHEMES
} hb_buffer_cluster_level_t;

HB_EXTERN void hb_buffer_set_cluster_level(hb_buffer_t *buffer, hb_buffer_cluster_level_t cluster_level);

HB_EXTERN hb_buffer_cluster_level_t hb_buffer_get_cluster_level(hb_buffer_t *buffer);

/**
 * HB_BUFFER_REPLACEMENT_CODEPOINT_DEFAULT:
 *
 * The default code point for replacing invalid characters in a given encoding.
 * Set to U+FFFD REPLACEMENT CHARACTER.
 *
 * Since: 0.9.31
 */
#define HB_BUFFER_REPLACEMENT_CODEPOINT_DEFAULT 0xFFFDu

HB_EXTERN void hb_buffer_set_replacement_codepoint(hb_buffer_t *buffer, hb_codepoint_t replacement);

HB_EXTERN hb_codepoint_t hb_buffer_get_replacement_codepoint(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_set_invisible_glyph(hb_buffer_t *buffer, hb_codepoint_t invisible);

HB_EXTERN hb_codepoint_t hb_buffer_get_invisible_glyph(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_reset(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_clear_contents(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_clear_output(hb_buffer_t *buffer);

HB_EXTERN hb_bool_t hb_buffer_pre_allocate(hb_buffer_t *buffer, unsigned int size);

HB_EXTERN hb_bool_t hb_buffer_allocation_successful(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_reverse(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_reverse_range(hb_buffer_t *buffer, unsigned int start, unsigned int end);

HB_EXTERN void hb_buffer_reverse_clusters(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_reset_clusters(hb_buffer_t *buffer);

/* Filling the buffer in */

HB_EXTERN void hb_buffer_add(hb_buffer_t *buffer, hb_codepoint_t codepoint, unsigned int cluster);

HB_EXTERN void
hb_buffer_add_utf8(hb_buffer_t *buffer, const char *text, int text_length, unsigned int item_offset, int item_length);

HB_EXTERN void hb_buffer_append(hb_buffer_t *buffer, hb_buffer_t *source, unsigned int start, unsigned int end);

HB_EXTERN hb_bool_t hb_buffer_set_length(hb_buffer_t *buffer, unsigned int length);

HB_EXTERN void hb_buffer_set_length_force(hb_buffer_t *buffer, unsigned int length);

HB_EXTERN unsigned int hb_buffer_get_length(hb_buffer_t *buffer);

HB_EXTERN unsigned int hb_buffer_get_out_length(hb_buffer_t *buffer);

HB_EXTERN unsigned int hb_buffer_get_allocated(hb_buffer_t *buffer);

HB_EXTERN hb_bool_t hb_buffer_ensure(hb_buffer_t *buffer, unsigned int length);

HB_EXTERN unsigned int hb_buffer_context_len(hb_buffer_t *buffer, unsigned int index);

HB_EXTERN hb_codepoint_t hb_buffer_context(hb_buffer_t *buffer, unsigned int context_index, unsigned int index);

/* Getting glyphs out of the buffer */

HB_EXTERN unsigned int hb_buffer_get_index(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_set_index(hb_buffer_t *buffer, unsigned int index);

HB_EXTERN hb_glyph_info_t *hb_buffer_get_glyph_infos(hb_buffer_t *buffer, unsigned int *length);

HB_EXTERN hb_glyph_info_t *hb_buffer_get_glyph_infos_ptr(hb_buffer_t *buffer);

HB_EXTERN hb_glyph_info_t *hb_buffer_get_out_glyph_infos_ptr(hb_buffer_t *buffer);

HB_EXTERN hb_glyph_position_t *hb_buffer_get_glyph_positions(hb_buffer_t *buffer, unsigned int *length);

HB_EXTERN hb_glyph_position_t *hb_buffer_get_glyph_positions_ptr(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_normalize_glyphs(hb_buffer_t *buffer);

HB_EXTERN unsigned int hb_buffer_get_scratch_flags(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_set_scratch_flags(hb_buffer_t *buffer, unsigned int flags);

HB_EXTERN void hb_buffer_next_glyph(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_replace_glyph(hb_buffer_t *buffer, const hb_codepoint_t glyph_index);

HB_EXTERN void hb_buffer_replace_glyphs(hb_buffer_t *buffer,
                                        unsigned int num_in,
                                        unsigned int num_out,
                                        const hb_codepoint_t *glyph_data);

HB_EXTERN void hb_buffer_output_glyph(hb_buffer_t *buffer, hb_codepoint_t glyph_index);

HB_EXTERN void hb_buffer_output_info(hb_buffer_t *buffer, hb_glyph_info_t ginfo);

HB_EXTERN void hb_buffer_unsafe_to_break(hb_buffer_t *buffer, unsigned int start, unsigned int end);

HB_EXTERN void hb_buffer_unsafe_to_break_from_outbuffer(hb_buffer_t *buffer, unsigned int start, unsigned int end);

HB_EXTERN void hb_buffer_merge_clusters(hb_buffer_t *buffer, unsigned int start, unsigned int end);

HB_EXTERN void hb_buffer_merge_out_clusters(hb_buffer_t *buffer, unsigned int start, unsigned int end);

HB_EXTERN void hb_buffer_swap_buffers(hb_buffer_t *buffer);

HB_EXTERN void hb_buffer_sort(hb_buffer_t *buffer, unsigned int start, unsigned int end, int (*compar)(const hb_glyph_info_t *, const hb_glyph_info_t *));

HB_END_DECLS

#endif /* HB_BUFFER_H */
