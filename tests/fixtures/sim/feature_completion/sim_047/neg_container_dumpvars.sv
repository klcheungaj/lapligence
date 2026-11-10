// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/neg_container_dumpvars.sv
// FIN-001 C4 negative: naming a queue in $dumpvars is the same located
// compile error whether the dump file is VCD or FST (S47-D9).
`ifndef FILE
`define FILE "dump.vcd"
`endif
module tb;
    int q[$];
    initial begin
        q.push_back(1);
        $dumpfile(`FILE);
        $dumpvars(1, q);
        $finish;
    end
endmodule
