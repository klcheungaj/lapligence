// llg-test-fixture: tests/fixtures/sim/memory_editions/invalid_address.sv
module tb;
    reg [7:0] mem [0:3];
    initial begin
        mem[0] = 8'h00;
        mem[1] = 8'h01;
        mem[2] = 8'h02;
        mem[3] = 8'h03;
        $readmemh("bad.mem", mem, 1, 0);
        $display("m0=%h m1=%h m2=%h m3=%h", mem[0], mem[1], mem[2], mem[3]);
    end
endmodule
