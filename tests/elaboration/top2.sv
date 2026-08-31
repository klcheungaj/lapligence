// Test 2: tougher elaboration features
// - conditional generate with parameter condition
// - multiple instances with different parameter values (uniquification)
// - localparam derived from parameters
// - initial blocks with delays
// - memories / arrays
// - enum types

package my_pkg;
    parameter int PKG_P = 3;
    typedef enum logic [1:0] { IDLE, RUN, DONE } state_t;
endpackage

module child #(
    parameter int W = 4,
    parameter int P = 1
) (
    input  logic [W-1:0] d,
    output logic [W-1:0] q
);
    localparam int W2 = W * 2;
    logic [W2-1:0] wide;
    assign q = d;

    genvar g;
    for (g = 0; g < P; g = g + 1) begin : gen_blk
        assign q[g] = d[g];
    end
endmodule

module cond_gen #(parameter int MODE = 0) (
    input logic a,
    output logic y
);
    if (MODE == 0) begin : gen0
        assign y = ~a;
    end else begin : gen1
        assign y = a;
    end
endmodule

module mem_mod #(parameter int DEPTH = 8, parameter int DW = 4) (
    input logic clk,
    input logic we,
    input logic [$clog2(DEPTH)-1:0] addr,
    input logic [DW-1:0] wdata,
    output logic [DW-1:0] rdata
);
    logic [DW-1:0] mem [0:DEPTH-1];
    always_ff @(posedge clk) begin
        if (we) mem[addr] <= wdata;
        rdata <= mem[addr];
    end
endmodule

module top2 (
    input  logic clk,
    input  logic a,
    input  logic we,
    input  logic [2:0] addr,
    input  logic [3:0] d4,
    input  logic [3:0] wdata,
    output logic [3:0] q4,
    output logic [3:0] rdata,
    output logic y0,
    output logic y1
);
    child #(.W(4), .P(2)) u_small (.d(d4), .q(q4));
    child #(.W(8), .P(1)) u_big (.d({d4, d4}), .q());

    cond_gen #(.MODE(0)) u_cg0 (.a(a), .y(y0));
    cond_gen #(.MODE(1)) u_cg1 (.a(a), .y(y1));

    mem_mod #(.DEPTH(8), .DW(4)) u_mem (
        .clk(clk), .we(we), .addr(addr), .wdata(wdata), .rdata(rdata)
    );

    my_pkg::state_t st;
    initial begin
        st = my_pkg::IDLE;
        #10;
        st = my_pkg::RUN;
        #10;
        st = my_pkg::DONE;
    end
endmodule
