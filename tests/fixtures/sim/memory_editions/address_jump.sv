// llg-test-fixture: tests/fixtures/sim/memory_editions/address_jump.sv
module tb;
    reg [7:0] mem [0:3];
    initial begin
        mem[0] = 0;
        mem[1] = 0;
        mem[2] = 0;
        mem[3] = 0;
        $readmemh("jumps.mem", mem, 3, 0);
        $display("m0=%h m1=%h m2=%h m3=%h", mem[0], mem[1], mem[2], mem[3]);
    end
endmodule
