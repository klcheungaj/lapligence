// The no-config package example (ST-50, ST-51): a header import of a package
// in another file, `default_nettype none, a macro guarded by `ifndef,
// `ifdef-guarded ports and nets and an instance of an unknown module.
`default_nettype none

`ifndef EX_DLY
`define EX_DLY 0
`endif

module ex_top
    import ex_pkg::*;
(
    input  wire       clk,
    input  wire       rst,
    input  wire       capture,
    input  wire       update,
    input  wire       shift,
    input  wire       tdi,
    output wire       tdo,
    input  wire [3:0] i_lane0,
    output wire [3:0] o_lane0,
`ifndef EX_NARROW
    input  wire [3:0] i_lane1,
    output wire [3:0] o_lane1,
`endif
    output wire       busy
);

`ifdef EX_NARROW
    wire [3:0] i_lane1;
    wire [3:0] o_lane1;
`endif

    mode_t                     mode = IDLE;
    mode_t                     next_mode;
    op_t                       op;
    op_args_t                  op_args;
    logic [FRAME_W-1:0]        frame = 0;
    logic [31:0]               count = 0;
    wire [WORD_W-1:0]          regs[(1<<IDX_W)-1:0];
    logic [IDX_W-1:0]          reg_sel = 0;
    logic [$clog2(N_LANES)-1:0] lane = 0;
    pass_args_t                pass_args = '0;
    sum_args_t                 sum_args = '0;
    wire                       sum_done;
    wire [31:0]                sum_value;

    assign op = op_t'(frame[FRAME_W-1 -: OP_W]);
    assign op_args = frame[FRAME_W-OP_W-1 -: ARG_W];

    always @(posedge clk or posedge rst) begin
        if (rst) begin
            frame <=#`EX_DLY '0;
        end else if (capture) begin
            frame <=#`EX_DLY {(FRAME_W - WORD_W)'(0), regs[reg_sel]};
        end else if (shift) begin
            frame <=#`EX_DLY {tdi, frame[FRAME_W-1:1]};
        end
    end

    always_comb begin
        next_mode = mode;
        case (mode)
            IDLE: if (update && op == OP_PASS) next_mode = PASS_PRE;
                  else if (update && op == OP_SUM) next_mode = SUM;
            PASS_PRE: if (count + 1 == pass_args.skip_bits) next_mode = PASS;
            PASS: if (update) next_mode = IDLE;
            SUM: if (sum_done || (update && op == OP_EXIT)) next_mode = IDLE;
            DRAIN: next_mode = IDLE;
            default: next_mode = IDLE;
        endcase
    end

    always_ff @(posedge clk or posedge rst) begin
        if (rst) begin
            mode <=#`EX_DLY IDLE;
            pass_args <=#`EX_DLY '0;
            sum_args <=#`EX_DLY '0;
            reg_sel <=#`EX_DLY '0;
        end else begin
            mode <=#`EX_DLY next_mode;
            if (update) begin
                case (op)
                    OP_PASS: pass_args <=#`EX_DLY op_args.pass;
                    OP_SUM: sum_args <=#`EX_DLY op_args.sum;
                    OP_READ, OP_WRITE: reg_sel <=#`EX_DLY op_args.rw.idx;
                    default: ;
                endcase
            end
            if (update && op == OP_WRITE && op_args.rw.idx == REG_LANE)
                lane <=#`EX_DLY op_args.rw.data;
            if (shift) count <=#`EX_DLY count + 1;
        end
    end

    assign regs[REG_VERSION] = 32'h0003_0000;
    assign regs[REG_MODE] = mode;
    assign regs[REG_LANE] = lane;
    assign regs[REG_OUT0] = sum_value;
    assign regs[REG_OUT1] = '0;

    ex_summer u_summer (
        .clk(clk),
        .rst(rst),
        .args(sum_args),
        .active(mode == SUM),
        .lane0(i_lane0),
        .lane1(i_lane1),
        .done(sum_done),
        .value(sum_value)
    );

    assign tdo = frame[0];
    assign busy = mode != IDLE;

    generate
        for (genvar i = 0; i < 4; i = i + 1) begin : g_out
            assign o_lane0[i] = mode == PASS && lane == 0 && tdi;
            assign o_lane1[i] = mode == PASS && lane == 1 && tdi;
        end
    endgenerate

endmodule
