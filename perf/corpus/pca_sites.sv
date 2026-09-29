`ifndef LLG_CORPUS_N
`define LLG_CORPUS_N 4100
`endif

// One small process per procedural continuous assignment site: the
// many-identical-instances shape of the 4,100-site capacity model. The
// compile-time ladder uses it to expose per-instance code growth.
module pca_sites #(
    parameter integer N = `LLG_CORPUS_N
);
    logic a = 1'b0;

    genvar i;
    for (i = 0; i < N; i = i + 1) begin : sites
        logic v;
        initial begin
            assign v = a;
        end
    end

    initial begin
        #1 a = 1'b1;
        #1;
        $display("pca_sites n=%0d first=%b last=%b", N, sites[0].v, sites[N-1].v);
        $finish(0);
    end
endmodule
