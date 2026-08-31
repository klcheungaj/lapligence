module param_child #(
    parameter int W = 4,
    parameter logic [3:0] INIT = 4'h0
) (
    output logic [W-1:0] o
);
    localparam int W2 = W + 1;
    localparam logic [W2-1:0] MASK = '1;
    localparam int DEPTH = 1 << W;
    localparam logic [7:0] EX = {2{INIT}};
    localparam int CB = $clog2(DEPTH);
    localparam logic [W2-1:0] SEL = W2 * 2;
    localparam logic [3:0] XZ = 4'b10xz;
    localparam int SNEG = -3;
    localparam int SNEG2 = SNEG + 1;
    assign o = INIT;
endmodule

module param_top (
    output logic [7:0] o0,
    output logic [15:0] o1
);
    param_child #(.W(8), .INIT(4'h5)) u0 (.o(o0));
    param_child #(.W(16), .INIT(4'ha)) u1 (.o(o1));
endmodule
