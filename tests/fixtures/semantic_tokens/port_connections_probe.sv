// The definition port_connections.sv instantiates, declared in another
// project file (ST-75, ST-76 in docs/semantic_tokens.md). It declares fewer
// ports than the instance connects, as a generated probe stub may.
module pc_probe (
    input logic        clk,
    input logic [31:0] bw_0,
    input logic [31:0] bw_1
);
endmodule
