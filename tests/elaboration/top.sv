// Test design to check Surelog elaboration completeness
// Features: parameter propagation, generate blocks, hierarchy, port connections

module counter #(
    parameter int WIDTH = 8,
    parameter logic [3:0] INIT = 4'h0
) (
    input  logic clk,
    input  logic rst_n,
    output logic [WIDTH-1:0] count,
    output logic done
);
    logic [WIDTH-1:0] next_count;
    assign next_count = count + 1'b1;
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) count <= INIT;
        else count <= next_count;
    end
    assign done = (count == '1);
endmodule

module adder #(parameter W = 4) (
    input  logic [W-1:0] a,
    input  logic [W-1:0] b,
    output logic [W:0]   sum
);
    assign sum = a + b;
endmodule

module gen_shift #(parameter N = 4) (
    input  logic [N-1:0] in,
    output logic [N-1:0] out
);
    genvar i;
    for (i = 0; i < N; i = i + 1) begin : g
        assign out[i] = in[i];
    end
endmodule

module top (
    input  logic clk,
    input  logic rst_n,
    input  logic [3:0] a,
    input  logic [3:0] b,
    output logic [7:0] count_out,
    output logic done,
    output logic [4:0] sum,
    output logic [3:0] shifted
);
    counter #(.WIDTH(8), .INIT(4'h5)) u_counter (
        .clk(clk),
        .rst_n(rst_n),
        .count(count_out),
        .done(done)
    );

    adder #(.W(4)) u_adder (
        .a(a),
        .b(b),
        .sum(sum)
    );

    gen_shift #(.N(4)) u_shift (
        .in(a),
        .out(shifted)
    );

    wire [3:0] mid;
    assign mid = a ^ b;
endmodule
