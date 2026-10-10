
/* ---- DPI-C import bridge and svdpi.h routines (SV Annex H) ----
 *
 * Embedded after the VPI bridge, which every model links, so the svdpi.h
 * routines are always present for user libraries. Thunks pass foreign code
 * buffers they own for one call; nothing here retains a pointer past it. */
#define DPI_PROTOTYPES
#define XXTERN DPI_EXTERN
#define EETERN DPI_EXTERN
#include "svdpi.h"

/* Chunks of a canonical vector of `width` bits (SV_PACKED_DATA_NELEMS). */
static size_t llg_dpi_chunks(uint64_t width) {
    return (size_t)((width + 31u) / 32u);
}

void llg_dpi_export_bits(const sv4_t* value, uint32_t* out, size_t count) {
    for (size_t i = 0; i < count; i += 2u) {
        llg_sv4_vpi_word_t word = llg_sv4_vpi_word(*value, i / 2u);
        /* A 2-state formal carries no unknowns; mask any defensively. */
        uint64_t bits = word.aval & ~word.bval;
        out[i] = (uint32_t)bits;
        if (i + 1u < count) out[i + 1u] = (uint32_t)(bits >> 32);
    }
}

void llg_dpi_export_logic(const sv4_t* value, void* out, size_t count) {
    llg_sv4_export_vpi32(*value, out, count, sizeof(svLogicVecVal));
}

void llg_dpi_import_bits(sv4_t* value, const uint32_t* in, size_t count) {
    size_t available = llg_dpi_chunks(llg_sv4_width(*value));
    if (count > available) count = available;
    for (size_t i = 0; i < count; i += 2u) {
        llg_sv4_vpi_word_t word = {in[i], 0};
        if (i + 1u < count) word.aval |= (uint64_t)in[i + 1u] << 32;
        llg_sv4_set_vpi_word(value, i / 2u, word);
    }
}

void llg_dpi_import_logic(sv4_t* value, const void* in, size_t count) {
    llg_sv4_import_vpi32(value, in, count, sizeof(svLogicVecVal));
}

void llg_dpi_fill_x(void* out, size_t count) {
    svLogicVecVal* chunks = (svLogicVecVal*)out;
    for (size_t i = 0; i < count; ++i) {
        chunks[i].aval = UINT32_MAX;
        chunks[i].bval = UINT32_MAX;
    }
}

/* Read `width` <= 32 bits at bit `lsb` of a canonical chunk array. */
static svLogicVecVal llg_dpi_get_field(const svLogicVecVal* chunks, uint64_t lsb, unsigned width) {
    size_t index = (size_t)(lsb / 32u);
    unsigned shift = (unsigned)(lsb % 32u);
    uint64_t aval = chunks[index].aval >> shift;
    uint64_t bval = chunks[index].bval >> shift;
    if (shift + width > 32u) {
        aval |= (uint64_t)chunks[index + 1u].aval << (32u - shift);
        bval |= (uint64_t)chunks[index + 1u].bval << (32u - shift);
    }
    uint64_t mask = width >= 32u ? UINT32_MAX : (((uint64_t)1 << width) - 1u);
    svLogicVecVal field = {(uint32_t)(aval & mask), (uint32_t)(bval & mask)};
    return field;
}

/* Write `width` <= 32 bits at bit `lsb` of a canonical chunk array. */
static void llg_dpi_set_field(svLogicVecVal* chunks, uint64_t lsb, unsigned width, svLogicVecVal field) {
    size_t index = (size_t)(lsb / 32u);
    unsigned shift = (unsigned)(lsb % 32u);
    uint64_t mask = width >= 32u ? UINT32_MAX : (((uint64_t)1 << width) - 1u);
    uint64_t aval = ((uint64_t)field.aval & mask) << shift;
    uint64_t bval = ((uint64_t)field.bval & mask) << shift;
    uint64_t keep = ~(mask << shift);
    chunks[index].aval = (uint32_t)((chunks[index].aval & keep) | aval);
    chunks[index].bval = (uint32_t)((chunks[index].bval & keep) | bval);
    if (shift + width > 32u) {
        uint64_t high_keep = ~(mask >> (32u - shift));
        chunks[index + 1u].aval = (uint32_t)((chunks[index + 1u].aval & high_keep) | (aval >> 32));
        chunks[index + 1u].bval = (uint32_t)((chunks[index + 1u].bval & high_keep) | (bval >> 32));
    }
}

/* Payload offset of the element at C index `k` of an array level: the
 * payload lists elements in declaration order (left bound first) from the
 * most significant end, the C layout lowest index first (H.7.3). */
static uint64_t llg_dpi_element_lsb(const llg_dpi_type_t* type, uint64_t lsb, size_t k) {
    size_t position = type->left <= type->right ? k : type->count - 1u - k;
    return lsb + (uint64_t)(type->count - 1u - position) * type->element->width;
}

static void llg_dpi_leaf_to_c(const llg_dpi_type_t* type, const svLogicVecVal* chunks,
                              uint64_t lsb, unsigned char* out) {
    switch (type->kind) {
    case LLG_DPI_BIT: {
        svLogicVecVal field = llg_dpi_get_field(chunks, lsb, 1u);
        *out = (svBit)(field.aval & ~field.bval & 1u);
        break;
    }
    case LLG_DPI_LOGIC: {
        svLogicVecVal field = llg_dpi_get_field(chunks, lsb, 1u);
        /* (aval, bval) 00/10/01/11 are sv_0/sv_1/sv_z/sv_x. */
        *out = (svLogic)((field.aval & 1u) | ((field.bval & 1u) << 1));
        break;
    }
    case LLG_DPI_INT: {
        uint64_t bits = 0;
        unsigned width = (unsigned)type->size * 8u;
        for (unsigned done = 0; done < width; done += 32u) {
            unsigned part = width - done < 32u ? width - done : 32u;
            svLogicVecVal field = llg_dpi_get_field(chunks, lsb + done, part);
            bits |= (uint64_t)(field.aval & ~field.bval) << done;
        }
        if (type->size == 1u) { uint8_t v = (uint8_t)bits; memcpy(out, &v, 1u); }
        else if (type->size == 2u) { uint16_t v = (uint16_t)bits; memcpy(out, &v, 2u); }
        else if (type->size == 4u) { uint32_t v = (uint32_t)bits; memcpy(out, &v, 4u); }
        else { memcpy(out, &bits, 8u); }
        break;
    }
    case LLG_DPI_BITVEC: {
        size_t count = llg_dpi_chunks(type->width);
        for (size_t i = 0; i < count; ++i) {
            unsigned part = type->width - i * 32u < 32u ? (unsigned)(type->width - i * 32u) : 32u;
            svLogicVecVal field = llg_dpi_get_field(chunks, lsb + i * 32u, part);
            svBitVecVal bits = field.aval & ~field.bval;
            memcpy(out + i * sizeof(svBitVecVal), &bits, sizeof(bits));
        }
        break;
    }
    case LLG_DPI_LOGICVEC: {
        size_t count = llg_dpi_chunks(type->width);
        for (size_t i = 0; i < count; ++i) {
            unsigned part = type->width - i * 32u < 32u ? (unsigned)(type->width - i * 32u) : 32u;
            svLogicVecVal field = llg_dpi_get_field(chunks, lsb + i * 32u, part);
            memcpy(out + i * sizeof(svLogicVecVal), &field, sizeof(field));
        }
        break;
    }
    default:
        break;
    }
}

static void llg_dpi_walk_to_c(const llg_dpi_type_t* type, const svLogicVecVal* chunks,
                              uint64_t lsb, unsigned char* out) {
    if (type->kind == LLG_DPI_ARRAY) {
        for (size_t k = 0; k < type->count; ++k)
            llg_dpi_walk_to_c(type->element, chunks, llg_dpi_element_lsb(type, lsb, k),
                              out + k * type->element->size);
    } else if (type->kind == LLG_DPI_STRUCT) {
        /* The first member is the most significant. */
        uint64_t member_lsb = lsb + type->width;
        for (size_t m = 0; m < type->count; ++m) {
            member_lsb -= type->members[m].type->width;
            llg_dpi_walk_to_c(type->members[m].type, chunks, member_lsb,
                              out + type->members[m].offset);
        }
    } else {
        llg_dpi_leaf_to_c(type, chunks, lsb, out);
    }
}

static void llg_dpi_leaf_from_c(const llg_dpi_type_t* type, const unsigned char* in,
                                svLogicVecVal* chunks, uint64_t lsb) {
    switch (type->kind) {
    case LLG_DPI_BIT: {
        svLogicVecVal field = {(uint32_t)(*in & 1u), 0};
        llg_dpi_set_field(chunks, lsb, 1u, field);
        break;
    }
    case LLG_DPI_LOGIC: {
        svLogicVecVal field = {(uint32_t)(*in & 1u), (uint32_t)((*in >> 1) & 1u)};
        llg_dpi_set_field(chunks, lsb, 1u, field);
        break;
    }
    case LLG_DPI_INT: {
        uint64_t bits = 0;
        if (type->size == 1u) { uint8_t v; memcpy(&v, in, 1u); bits = v; }
        else if (type->size == 2u) { uint16_t v; memcpy(&v, in, 2u); bits = v; }
        else if (type->size == 4u) { uint32_t v; memcpy(&v, in, 4u); bits = v; }
        else { memcpy(&bits, in, 8u); }
        unsigned width = (unsigned)type->size * 8u;
        for (unsigned done = 0; done < width; done += 32u) {
            unsigned part = width - done < 32u ? width - done : 32u;
            svLogicVecVal field = {(uint32_t)(bits >> done), 0};
            llg_dpi_set_field(chunks, lsb + done, part, field);
        }
        break;
    }
    case LLG_DPI_BITVEC: {
        size_t count = llg_dpi_chunks(type->width);
        for (size_t i = 0; i < count; ++i) {
            unsigned part = type->width - i * 32u < 32u ? (unsigned)(type->width - i * 32u) : 32u;
            svBitVecVal bits;
            memcpy(&bits, in + i * sizeof(svBitVecVal), sizeof(bits));
            svLogicVecVal field = {bits, 0};
            llg_dpi_set_field(chunks, lsb + i * 32u, part, field);
        }
        break;
    }
    case LLG_DPI_LOGICVEC: {
        size_t count = llg_dpi_chunks(type->width);
        for (size_t i = 0; i < count; ++i) {
            unsigned part = type->width - i * 32u < 32u ? (unsigned)(type->width - i * 32u) : 32u;
            svLogicVecVal field;
            memcpy(&field, in + i * sizeof(svLogicVecVal), sizeof(field));
            llg_dpi_set_field(chunks, lsb + i * 32u, part, field);
        }
        break;
    }
    default:
        break;
    }
}

static void llg_dpi_walk_from_c(const llg_dpi_type_t* type, const unsigned char* in,
                                svLogicVecVal* chunks, uint64_t lsb) {
    if (type->kind == LLG_DPI_ARRAY) {
        for (size_t k = 0; k < type->count; ++k)
            llg_dpi_walk_from_c(type->element, in + k * type->element->size, chunks,
                                llg_dpi_element_lsb(type, lsb, k));
    } else if (type->kind == LLG_DPI_STRUCT) {
        uint64_t member_lsb = lsb + type->width;
        for (size_t m = 0; m < type->count; ++m) {
            member_lsb -= type->members[m].type->width;
            llg_dpi_walk_from_c(type->members[m].type, in + type->members[m].offset, chunks,
                                member_lsb);
        }
    } else {
        llg_dpi_leaf_from_c(type, in, chunks, lsb);
    }
}

static svLogicVecVal* llg_dpi_scratch(size_t count) {
    svLogicVecVal* chunks = (svLogicVecVal*)calloc(count ? count : 1u, sizeof(svLogicVecVal));
    if (!chunks) {
        fprintf(stderr, "llg: out of memory converting a DPI-C argument\n");
        abort();
    }
    return chunks;
}

void llg_dpi_to_c(const llg_dpi_type_t* type, const sv4_t* payload, void* out) {
    size_t count = llg_dpi_chunks(type->width);
    svLogicVecVal* chunks = llg_dpi_scratch(count);
    llg_sv4_export_vpi32(*payload, chunks, count, sizeof(svLogicVecVal));
    memset(out, 0, type->size);
    llg_dpi_walk_to_c(type, chunks, 0, (unsigned char*)out);
    free(chunks);
}

void llg_dpi_from_c(const llg_dpi_type_t* type, const void* in, sv4_t* payload) {
    size_t count = llg_dpi_chunks(type->width);
    svLogicVecVal* chunks = llg_dpi_scratch(count);
    llg_dpi_walk_from_c(type, (const unsigned char*)in, chunks, 0);
    llg_sv4_import_vpi32(payload, chunks, count, sizeof(svLogicVecVal));
    free(chunks);
}

static void llg_dpi_walk_init(const llg_dpi_type_t* type, unsigned char* out) {
    switch (type->kind) {
    case LLG_DPI_ARRAY:
        for (size_t k = 0; k < type->count; ++k)
            llg_dpi_walk_init(type->element, out + k * type->element->size);
        break;
    case LLG_DPI_STRUCT:
        for (size_t m = 0; m < type->count; ++m)
            llg_dpi_walk_init(type->members[m].type, out + type->members[m].offset);
        break;
    case LLG_DPI_LOGIC:
        *out = sv_x;
        break;
    case LLG_DPI_LOGICVEC:
        llg_dpi_fill_x(out, llg_dpi_chunks(type->width));
        break;
    default:
        break;
    }
}

void llg_dpi_init_c(const llg_dpi_type_t* type, void* out) {
    memset(out, 0, type->size);
    llg_dpi_walk_init(type, (unsigned char*)out);
}

/* ---- svdpi.h: version and canonical bit/part selects (H.10.1.3, H.11.5) */

const char* svDpiVersion(void) {
    return "1800-2005";
}

svBit svGetBitselBit(const svBitVecVal* s, int i) {
    if (!s || i < 0) return sv_0;
    return (svBit)((s[i / 32] >> (i % 32)) & 1u);
}

svLogic svGetBitselLogic(const svLogicVecVal* s, int i) {
    if (!s || i < 0) return sv_x;
    uint32_t a = (s[i / 32].aval >> (i % 32)) & 1u;
    uint32_t b = (s[i / 32].bval >> (i % 32)) & 1u;
    return (svLogic)(a | (b << 1));
}

void svPutBitselBit(svBitVecVal* d, int i, svBit s) {
    if (!d || i < 0) return;
    uint32_t mask = (uint32_t)1u << (i % 32);
    d[i / 32] = (s & 1u) ? (d[i / 32] | mask) : (d[i / 32] & ~mask);
}

void svPutBitselLogic(svLogicVecVal* d, int i, svLogic s) {
    if (!d || i < 0) return;
    uint32_t mask = (uint32_t)1u << (i % 32);
    d[i / 32].aval = (s & 1u) ? (d[i / 32].aval | mask) : (d[i / 32].aval & ~mask);
    d[i / 32].bval = (s & 2u) ? (d[i / 32].bval | mask) : (d[i / 32].bval & ~mask);
}

/* Part selects move 1..32 bits; other widths and negative indices are
 * undetermined (H.11.5) and leave the destination unchanged. */
void svGetPartselBit(svBitVecVal* d, const svBitVecVal* s, int i, int w) {
    if (!d || !s || i < 0 || w <= 0 || w > 32) return;
    uint64_t bits = s[i / 32] >> (i % 32);
    if (i % 32 + w > 32) bits |= (uint64_t)s[i / 32 + 1] << (32 - i % 32);
    uint64_t mask = w == 32 ? UINT32_MAX : (((uint64_t)1 << w) - 1u);
    *d = (svBitVecVal)((*d & ~mask) | (bits & mask));
}

void svGetPartselLogic(svLogicVecVal* d, const svLogicVecVal* s, int i, int w) {
    if (!d || !s || i < 0 || w <= 0 || w > 32) return;
    svLogicVecVal field = llg_dpi_get_field(s, (uint64_t)i, (unsigned)w);
    uint64_t mask = w == 32 ? UINT32_MAX : (((uint64_t)1 << w) - 1u);
    d->aval = (uint32_t)((d->aval & ~mask) | field.aval);
    d->bval = (uint32_t)((d->bval & ~mask) | field.bval);
}

void svPutPartselBit(svBitVecVal* d, const svBitVecVal s, int i, int w) {
    if (!d || i < 0 || w <= 0 || w > 32) return;
    svLogicVecVal field = {s, 0};
    svLogicVecVal pair[2];
    size_t index = (size_t)i / 32u;
    size_t span = (size_t)(i % 32 + w > 32 ? 2 : 1);
    for (size_t k = 0; k < span; ++k) {
        pair[k].aval = d[index + k];
        pair[k].bval = 0;
    }
    llg_dpi_set_field(pair, (uint64_t)(i % 32), (unsigned)w, field);
    for (size_t k = 0; k < span; ++k) d[index + k] = pair[k].aval;
}

void svPutPartselLogic(svLogicVecVal* d, const svLogicVecVal s, int i, int w) {
    if (!d || i < 0 || w <= 0 || w > 32) return;
    llg_dpi_set_field(d, (uint64_t)i, (unsigned)w, s);
}

/* ---- svdpi.h: open arrays (H.12) ---- */

/* The array level of dimension `d` (1-based, slowest first), or NULL. */
static const llg_dpi_type_t* llg_dpi_level(const llg_dpi_open_t* open, int d) {
    if (!open || d < 1 || d > open->dims) return NULL;
    const llg_dpi_type_t* level = open->type;
    for (int i = 1; i < d; ++i) level = level->element;
    return level;
}

/* The element below every unpacked dimension. */
static const llg_dpi_type_t* llg_dpi_open_element(const llg_dpi_open_t* open) {
    const llg_dpi_type_t* element = open->type;
    for (int i = 0; i < open->dims; ++i) element = element->element;
    return element;
}

/* Packed width of an element with a packed part (dimension 0), else 0. */
static uint32_t llg_dpi_packed_width(const llg_dpi_type_t* element) {
    switch (element->kind) {
    case LLG_DPI_BIT:
    case LLG_DPI_LOGIC:
    case LLG_DPI_INT:
    case LLG_DPI_BITVEC:
    case LLG_DPI_LOGICVEC:
        return element->width;
    default:
        return 0;
    }
}

/* Dimension `d` as [left:right]; 0 for the packed part, linearized and
 * normalized (H.12.2). Returns 0 when `d` names no dimension. */
static int llg_dpi_range(const svOpenArrayHandle h, int d, int* left, int* right) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!open) return 0;
    if (d == 0) {
        uint32_t width = llg_dpi_packed_width(llg_dpi_open_element(open));
        if (!width) return 0;
        *left = (int)width - 1;
        *right = 0;
        return 1;
    }
    const llg_dpi_type_t* level = llg_dpi_level(open, d);
    if (!level) return 0;
    *left = level->left;
    *right = level->right;
    return 1;
}

int svLeft(const svOpenArrayHandle h, int d) {
    int left = 0, right = 0;
    return llg_dpi_range(h, d, &left, &right) ? left : 0;
}

int svRight(const svOpenArrayHandle h, int d) {
    int left = 0, right = 0;
    return llg_dpi_range(h, d, &left, &right) ? right : 0;
}

int svLow(const svOpenArrayHandle h, int d) {
    int left = 0, right = 0;
    if (!llg_dpi_range(h, d, &left, &right)) return 0;
    return left < right ? left : right;
}

int svHigh(const svOpenArrayHandle h, int d) {
    int left = 0, right = 0;
    if (!llg_dpi_range(h, d, &left, &right)) return 0;
    return left > right ? left : right;
}

int svIncrement(const svOpenArrayHandle h, int d) {
    int left = 0, right = 0;
    if (!llg_dpi_range(h, d, &left, &right)) return 0;
    return left >= right ? 1 : -1;
}

int svSize(const svOpenArrayHandle h, int d) {
    int left = 0, right = 0;
    if (!llg_dpi_range(h, d, &left, &right)) return 0;
    return (left > right ? left - right : right - left) + 1;
}

int svDimensions(const svOpenArrayHandle h) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!open) return 0;
    return open->dims + (llg_dpi_packed_width(llg_dpi_open_element(open)) ? 1 : 0);
}

void* svGetArrayPtr(const svOpenArrayHandle h) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    return open ? open->data : NULL;
}

int svSizeOfArray(const svOpenArrayHandle h) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    return open && open->type->size <= (size_t)INT32_MAX ? (int)open->type->size : 0;
}

/* Element address for exactly `count` original-range indices, else NULL. */
static unsigned char* llg_dpi_element_at(const svOpenArrayHandle h, int count, const int* index) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!open || count != open->dims || count < 1) return NULL;
    const llg_dpi_type_t* level = open->type;
    unsigned char* address = (unsigned char*)open->data;
    for (int i = 0; i < count; ++i) {
        int64_t low = level->left < level->right ? level->left : level->right;
        int64_t high = level->left > level->right ? level->left : level->right;
        if (index[i] < low || index[i] > high) return NULL;
        address += (size_t)(index[i] - low) * level->element->size;
        level = level->element;
    }
    return address;
}

static unsigned char* llg_dpi_element_va(const svOpenArrayHandle h, int first, va_list rest) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    int index[64];
    if (!open || open->dims < 1 || open->dims > 64) return NULL;
    index[0] = first;
    for (int i = 1; i < open->dims; ++i) index[i] = va_arg(rest, int);
    return llg_dpi_element_at(h, open->dims, index);
}

void* svGetArrElemPtr(const svOpenArrayHandle h, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    unsigned char* address = llg_dpi_element_va(h, indx1, rest);
    va_end(rest);
    return address;
}

void* svGetArrElemPtr1(const svOpenArrayHandle h, int indx1) {
    int index[1] = {indx1};
    return llg_dpi_element_at(h, 1, index);
}

void* svGetArrElemPtr2(const svOpenArrayHandle h, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    return llg_dpi_element_at(h, 2, index);
}

void* svGetArrElemPtr3(const svOpenArrayHandle h, int indx1, int indx2, int indx3) {
    int index[3] = {indx1, indx2, indx3};
    return llg_dpi_element_at(h, 3, index);
}

/* Copy one packed element to canonical chunks. An unknown bit read as 2-state
 * is 0; an invalid index reads the element type's default (SV 7.4.6). */
static void llg_dpi_get_vec(const svOpenArrayHandle h, const unsigned char* element,
                            svLogicVecVal* out, int logic) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!open) return;
    const llg_dpi_type_t* type = llg_dpi_open_element(open);
    uint32_t width = llg_dpi_packed_width(type);
    if (!width) return;
    size_t count = llg_dpi_chunks(width);
    int four_state = type->kind == LLG_DPI_LOGIC || type->kind == LLG_DPI_LOGICVEC;
    if (!element) {
        for (size_t i = 0; i < count; ++i) {
            out[i].aval = four_state ? UINT32_MAX : 0u;
            out[i].bval = four_state ? UINT32_MAX : 0u;
        }
    } else {
        memset(out, 0, count * sizeof(svLogicVecVal));
        llg_dpi_leaf_from_c(type, element, out, 0);
        if (width % 32u) {
            uint32_t mask = ((uint32_t)1u << (width % 32u)) - 1u;
            out[count - 1u].aval &= mask;
            out[count - 1u].bval &= mask;
        }
    }
    if (!logic)
        for (size_t i = 0; i < count; ++i) out[i].aval &= ~out[i].bval, out[i].bval = 0;
}

static void llg_dpi_put_vec(const svOpenArrayHandle h, unsigned char* element,
                            const svLogicVecVal* in) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!open || !element) return;
    const llg_dpi_type_t* type = llg_dpi_open_element(open);
    uint32_t width = llg_dpi_packed_width(type);
    if (!width) return;
    size_t count = llg_dpi_chunks(width);
    svLogicVecVal* chunks = llg_dpi_scratch(count);
    memcpy(chunks, in, count * sizeof(svLogicVecVal));
    if (type->kind != LLG_DPI_LOGIC && type->kind != LLG_DPI_LOGICVEC)
        for (size_t i = 0; i < count; ++i) chunks[i].aval &= ~chunks[i].bval, chunks[i].bval = 0;
    llg_dpi_leaf_to_c(type, chunks, 0, element);
    free(chunks);
}

static void llg_dpi_get_bits(svBitVecVal* d, const svOpenArrayHandle h, const unsigned char* element) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!d || !open) return;
    size_t count = llg_dpi_chunks(llg_dpi_packed_width(llg_dpi_open_element(open)));
    if (!count) return;
    svLogicVecVal* chunks = llg_dpi_scratch(count);
    llg_dpi_get_vec(h, element, chunks, 0);
    for (size_t i = 0; i < count; ++i) d[i] = chunks[i].aval;
    free(chunks);
}

static void llg_dpi_put_bits(const svOpenArrayHandle h, unsigned char* element, const svBitVecVal* s) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!s || !open || !element) return;
    size_t count = llg_dpi_chunks(llg_dpi_packed_width(llg_dpi_open_element(open)));
    if (!count) return;
    svLogicVecVal* chunks = llg_dpi_scratch(count);
    for (size_t i = 0; i < count; ++i) chunks[i].aval = s[i], chunks[i].bval = 0;
    llg_dpi_put_vec(h, element, chunks);
    free(chunks);
}

void svPutBitArrElemVecVal(const svOpenArrayHandle d, const svBitVecVal* s, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    llg_dpi_put_bits(d, llg_dpi_element_va(d, indx1, rest), s);
    va_end(rest);
}

void svPutBitArrElem1VecVal(const svOpenArrayHandle d, const svBitVecVal* s, int indx1) {
    int index[1] = {indx1};
    llg_dpi_put_bits(d, llg_dpi_element_at(d, 1, index), s);
}

void svPutBitArrElem2VecVal(const svOpenArrayHandle d, const svBitVecVal* s, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    llg_dpi_put_bits(d, llg_dpi_element_at(d, 2, index), s);
}

void svPutBitArrElem3VecVal(const svOpenArrayHandle d, const svBitVecVal* s, int indx1, int indx2,
                            int indx3) {
    int index[3] = {indx1, indx2, indx3};
    llg_dpi_put_bits(d, llg_dpi_element_at(d, 3, index), s);
}

void svPutLogicArrElemVecVal(const svOpenArrayHandle d, const svLogicVecVal* s, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    if (s) llg_dpi_put_vec(d, llg_dpi_element_va(d, indx1, rest), s);
    va_end(rest);
}

void svPutLogicArrElem1VecVal(const svOpenArrayHandle d, const svLogicVecVal* s, int indx1) {
    int index[1] = {indx1};
    if (s) llg_dpi_put_vec(d, llg_dpi_element_at(d, 1, index), s);
}

void svPutLogicArrElem2VecVal(const svOpenArrayHandle d, const svLogicVecVal* s, int indx1,
                              int indx2) {
    int index[2] = {indx1, indx2};
    if (s) llg_dpi_put_vec(d, llg_dpi_element_at(d, 2, index), s);
}

void svPutLogicArrElem3VecVal(const svOpenArrayHandle d, const svLogicVecVal* s, int indx1,
                              int indx2, int indx3) {
    int index[3] = {indx1, indx2, indx3};
    if (s) llg_dpi_put_vec(d, llg_dpi_element_at(d, 3, index), s);
}

void svGetBitArrElemVecVal(svBitVecVal* d, const svOpenArrayHandle s, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    llg_dpi_get_bits(d, s, llg_dpi_element_va(s, indx1, rest));
    va_end(rest);
}

void svGetBitArrElem1VecVal(svBitVecVal* d, const svOpenArrayHandle s, int indx1) {
    int index[1] = {indx1};
    llg_dpi_get_bits(d, s, llg_dpi_element_at(s, 1, index));
}

void svGetBitArrElem2VecVal(svBitVecVal* d, const svOpenArrayHandle s, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    llg_dpi_get_bits(d, s, llg_dpi_element_at(s, 2, index));
}

void svGetBitArrElem3VecVal(svBitVecVal* d, const svOpenArrayHandle s, int indx1, int indx2,
                            int indx3) {
    int index[3] = {indx1, indx2, indx3};
    llg_dpi_get_bits(d, s, llg_dpi_element_at(s, 3, index));
}

void svGetLogicArrElemVecVal(svLogicVecVal* d, const svOpenArrayHandle s, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    if (d) llg_dpi_get_vec(s, llg_dpi_element_va(s, indx1, rest), d, 1);
    va_end(rest);
}

void svGetLogicArrElem1VecVal(svLogicVecVal* d, const svOpenArrayHandle s, int indx1) {
    int index[1] = {indx1};
    if (d) llg_dpi_get_vec(s, llg_dpi_element_at(s, 1, index), d, 1);
}

void svGetLogicArrElem2VecVal(svLogicVecVal* d, const svOpenArrayHandle s, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    if (d) llg_dpi_get_vec(s, llg_dpi_element_at(s, 2, index), d, 1);
}

void svGetLogicArrElem3VecVal(svLogicVecVal* d, const svOpenArrayHandle s, int indx1, int indx2,
                              int indx3) {
    int index[3] = {indx1, indx2, indx3};
    if (d) llg_dpi_get_vec(s, llg_dpi_element_at(s, 3, index), d, 1);
}

/* Scalar element access (H.12.6) reads bit 0 of the element. */
static svLogic llg_dpi_get_scalar(const svOpenArrayHandle h, const unsigned char* element, int logic) {
    svLogicVecVal chunks[2];
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!open || !llg_dpi_packed_width(llg_dpi_open_element(open))) return logic ? sv_x : sv_0;
    if (llg_dpi_chunks(llg_dpi_packed_width(llg_dpi_open_element(open))) > 1u) {
        svLogicVecVal* wide = llg_dpi_scratch(llg_dpi_chunks(llg_dpi_packed_width(llg_dpi_open_element(open))));
        llg_dpi_get_vec(h, element, wide, logic);
        chunks[0] = wide[0];
        free(wide);
    } else {
        llg_dpi_get_vec(h, element, chunks, logic);
    }
    return (svLogic)((chunks[0].aval & 1u) | ((chunks[0].bval & 1u) << 1));
}

static void llg_dpi_put_scalar(const svOpenArrayHandle h, unsigned char* element, svLogic value) {
    const llg_dpi_open_t* open = (const llg_dpi_open_t*)h;
    if (!open || !element) return;
    uint32_t width = llg_dpi_packed_width(llg_dpi_open_element(open));
    if (!width) return;
    size_t count = llg_dpi_chunks(width);
    svLogicVecVal* chunks = llg_dpi_scratch(count);
    llg_dpi_get_vec(h, element, chunks, 1);
    chunks[0].aval = (chunks[0].aval & ~1u) | (value & 1u);
    chunks[0].bval = (chunks[0].bval & ~1u) | ((value >> 1) & 1u);
    llg_dpi_put_vec(h, element, chunks);
    free(chunks);
}

svBit svGetBitArrElem(const svOpenArrayHandle s, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    svLogic value = llg_dpi_get_scalar(s, llg_dpi_element_va(s, indx1, rest), 0);
    va_end(rest);
    return (svBit)value;
}

svBit svGetBitArrElem1(const svOpenArrayHandle s, int indx1) {
    int index[1] = {indx1};
    return (svBit)llg_dpi_get_scalar(s, llg_dpi_element_at(s, 1, index), 0);
}

svBit svGetBitArrElem2(const svOpenArrayHandle s, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    return (svBit)llg_dpi_get_scalar(s, llg_dpi_element_at(s, 2, index), 0);
}

svBit svGetBitArrElem3(const svOpenArrayHandle s, int indx1, int indx2, int indx3) {
    int index[3] = {indx1, indx2, indx3};
    return (svBit)llg_dpi_get_scalar(s, llg_dpi_element_at(s, 3, index), 0);
}

svLogic svGetLogicArrElem(const svOpenArrayHandle s, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    svLogic value = llg_dpi_get_scalar(s, llg_dpi_element_va(s, indx1, rest), 1);
    va_end(rest);
    return value;
}

svLogic svGetLogicArrElem1(const svOpenArrayHandle s, int indx1) {
    int index[1] = {indx1};
    return llg_dpi_get_scalar(s, llg_dpi_element_at(s, 1, index), 1);
}

svLogic svGetLogicArrElem2(const svOpenArrayHandle s, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    return llg_dpi_get_scalar(s, llg_dpi_element_at(s, 2, index), 1);
}

svLogic svGetLogicArrElem3(const svOpenArrayHandle s, int indx1, int indx2, int indx3) {
    int index[3] = {indx1, indx2, indx3};
    return llg_dpi_get_scalar(s, llg_dpi_element_at(s, 3, index), 1);
}

void svPutLogicArrElem(const svOpenArrayHandle d, svLogic value, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    llg_dpi_put_scalar(d, llg_dpi_element_va(d, indx1, rest), value);
    va_end(rest);
}

void svPutLogicArrElem1(const svOpenArrayHandle d, svLogic value, int indx1) {
    int index[1] = {indx1};
    llg_dpi_put_scalar(d, llg_dpi_element_at(d, 1, index), value);
}

void svPutLogicArrElem2(const svOpenArrayHandle d, svLogic value, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    llg_dpi_put_scalar(d, llg_dpi_element_at(d, 2, index), value);
}

void svPutLogicArrElem3(const svOpenArrayHandle d, svLogic value, int indx1, int indx2, int indx3) {
    int index[3] = {indx1, indx2, indx3};
    llg_dpi_put_scalar(d, llg_dpi_element_at(d, 3, index), value);
}

void svPutBitArrElem(const svOpenArrayHandle d, svBit value, int indx1, ...) {
    va_list rest;
    va_start(rest, indx1);
    llg_dpi_put_scalar(d, llg_dpi_element_va(d, indx1, rest), (svLogic)(value & 1u));
    va_end(rest);
}

void svPutBitArrElem1(const svOpenArrayHandle d, svBit value, int indx1) {
    int index[1] = {indx1};
    llg_dpi_put_scalar(d, llg_dpi_element_at(d, 1, index), (svLogic)(value & 1u));
}

void svPutBitArrElem2(const svOpenArrayHandle d, svBit value, int indx1, int indx2) {
    int index[2] = {indx1, indx2};
    llg_dpi_put_scalar(d, llg_dpi_element_at(d, 2, index), (svLogic)(value & 1u));
}

void svPutBitArrElem3(const svOpenArrayHandle d, svBit value, int indx1, int indx2, int indx3) {
    int index[3] = {indx1, indx2, indx3};
    llg_dpi_put_scalar(d, llg_dpi_element_at(d, 3, index), (svLogic)(value & 1u));
}
