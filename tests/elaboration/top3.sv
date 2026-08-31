// Test 3: localparams, 4-state, hierarchical refs, functions, case/for

module func_mod #(parameter int W = 4) (
    input logic [W-1:0] a,
    input logic [W-1:0] b,
    output logic [W-1:0] r
);
    localparam int W2 = W + 1;
    localparam logic [3:0] MASK = 4'b10xz;

    function automatic logic [W2-1:0] add_one(input logic [W2-1:0] v);
        add_one = v + 1;
    endfunction

    always_comb begin
        r = add_one(a) + b[W-1:0];
    end
endmodule

module hier_ref (
    input logic clk,
    output logic [3:0] o
);
    reg [3:0] inner;
    always @(posedge clk) inner <= inner + 1;
    assign o = inner;
endmodule

module tb (
    input logic clk,
    output logic [3:0] o
);
    // hierarchical reference from tb into child
    hier_ref u_hier (.clk(clk), .o(o));
    reg [3:0] captured;
    always @(posedge clk) captured <= u_hier.inner;

    reg [3:0] cnt;
    integer i;
    initial begin
        cnt = 0;
        for (i = 0; i < 4; i = i + 1) begin
            cnt = cnt + 1;
            #5;
        end
    end

    always @(cnt) begin
        case (cnt)
            4'd0: o = 4'bzzzz;
            4'd1: o = 4'd1;
            default: o = 4'hx;
        endcase
    end
endmodule
