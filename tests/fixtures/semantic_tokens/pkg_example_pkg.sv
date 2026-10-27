// Package of the no-config package example (ST-50, ST-51): the shape of a
// user report, anonymised. `pkg_example_top.sv` imports it in its header.
package ex_pkg;

    localparam FRAME_W = 64;  // width of the shift frame
    localparam OP_W = 6;  // width of an operation code
    localparam WORD_W = 32;  // width of a register
    localparam ARG_W = FRAME_W - OP_W;  // width of the operation arguments
    localparam IDX_W = 8;  // width of a register index
    localparam N_LANES = 8;

    typedef enum logic [2:0] {
        IDLE = 0,
        PASS_PRE = 1,
        PASS = 2,
        SUM = 3,
        DRAIN = 4
    } mode_t;

    typedef enum logic [OP_W-1:0] {
        OP_NONE = 0,
        OP_RESET = 1,
        OP_READ = 2,
        OP_WRITE = 3,
        OP_PASS = 4,
        OP_SUM = 5,
        OP_EXIT = 1 << (OP_W - 1)  // leave any mode
    } op_t;

    typedef struct packed {
        logic wide;
        logic [3:0] width;
        logic [7:0] lane_mask;
        logic [ARG_W-13-1:0] unused;
    } sum_args_t;

    typedef struct packed {
        logic [IDX_W-1:0] idx;
        logic [WORD_W-1:0] data;
        logic [ARG_W-WORD_W-IDX_W-1:0] unused;
    } rw_args_t;

    typedef struct packed {
        logic [7:0] skip_bits;
        logic [ARG_W-8-1:0] unused;
    } pass_args_t;

    typedef union packed {
        rw_args_t          rw;
        sum_args_t         sum;
        pass_args_t        pass;
        logic [ARG_W-1:0]  raw;
    } op_args_t;

    typedef enum logic [IDX_W-1:0] {
        REG_VERSION = 0,
        REG_MODE = 1,
        REG_LANE = 2,
        REG_OUT0 = 3,
        REG_OUT1 = 4
    } reg_t;

endpackage
