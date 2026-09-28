`ifndef LLG_CORPUS_N
`define LLG_CORPUS_N 10000
`endif

`ifndef LLG_CORPUS_EDGES
`define LLG_CORPUS_EDGES 20
`endif

module many_processes_registers #(
    parameter integer N = `LLG_CORPUS_N,
    parameter integer EDGES = `LLG_CORPUS_EDGES
);
    logic clk = 0;
    logic q [N-1:0];
    wire d [N-1:0];
    integer ones;
    integer d_ones;
    integer index;

    genvar i;
    for (i = 0; i < N; i = i + 1) begin : workers
        assign d[i] = i[0];
        always @(posedge clk)
            q[i] <= d[i];
    end

    initial begin
        repeat (EDGES) begin
            #1 clk = 1;
            #1 clk = 0;
        end
        #1;
        ones = 0;
        d_ones = 0;
        for (index = 0; index < N; index = index + 1) begin
            ones = ones + (q[index] === 1'b1);
            d_ones = d_ones + (d[index] === 1'b1);
        end
        $display("many_processes variant=registers n=%0d edges=%0d ones=%0d d_ones=%0d first=%b last=%b",
                 N, EDGES, ones, d_ones, q[0], q[N-1]);
        $finish(0);
    end
endmodule

// All processes update bit selections of the same wide value. This exercises
// the full-width mask/value representation used by the current NBA runtime.
module many_processes_masked #(
    parameter integer N = `LLG_CORPUS_N,
    parameter integer EDGES = `LLG_CORPUS_EDGES
);
    logic clk = 0;
    logic [N-1:0] q = '0;
    wire [N-1:0] d;

    genvar i;
    for (i = 0; i < N; i = i + 1) begin : workers
        assign d[i] = i[0];
        always @(posedge clk)
            q[i] <= d[i];
    end

    initial begin
        repeat (EDGES) begin
            #1 clk = 1;
            #1 clk = 0;
        end
        #1;
        $display("many_processes variant=masked n=%0d edges=%0d ones=%0d first=%b last=%b",
                 N, EDGES, $countones(q), q[0], q[N-1]);
        $finish(0);
    end
endmodule

// llg has no command-line top-parameter override. These small tops keep the
// workload parameterized while allowing the runner to choose values with -D.
module many_processes_registers_config;
    many_processes_registers #(
        .N(`LLG_CORPUS_N),
        .EDGES(`LLG_CORPUS_EDGES)
    ) corpus();
endmodule

module many_processes_masked_config;
    many_processes_masked #(
        .N(`LLG_CORPUS_N),
        .EDGES(`LLG_CORPUS_EDGES)
    ) corpus();
endmodule
