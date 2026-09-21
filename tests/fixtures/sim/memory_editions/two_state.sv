// llg-test-fixture: tests/fixtures/sim/memory_editions/two_state.sv
module tb;
    bit [3:0] mem [0:1];
    initial begin
        $readmemb("unknown.mem", mem);
        $display("m0=%b m1=%b", mem[0], mem[1]);
    end
endmodule
