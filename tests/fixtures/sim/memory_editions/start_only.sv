// llg-test-fixture: tests/fixtures/sim/memory_editions/start_only.sv
module tb;
    reg [7:0] mem [3:0];
    initial begin
        mem[3] = 0;
        mem[2] = 0;
        mem[1] = 0;
        mem[0] = 0;
        $readmemh("words.mem", mem, 2);
        $display("m3=%h m2=%h m1=%h m0=%h", mem[3], mem[2], mem[1], mem[0]);
    end
endmodule
