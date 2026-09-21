// llg-test-fixture: tests/fixtures/sim/memory_editions/truncated.sv
module tb;
    reg [7:0] mem [0:1];
    initial begin
        mem[0] = 8'ha0;
        mem[1] = 8'hb1;
        $readmemh("short.mem", mem, 0, 1);
        $display("m0=%h m1=%h", mem[0], mem[1]);
    end
endmodule
