// llg-test-fixture: tests/fixtures/sim/memory_views/negative_ranges.sv
module tb;
    logic [7:0] mem [3:1][-1:-2];
    initial begin
        mem[1][-2] = 8'hee;
        mem[1][-1] = 8'hee;
        mem[2][-2] = 8'hee;
        mem[2][-1] = 8'hee;
        mem[3][-2] = 8'hee;
        mem[3][-1] = 8'hee;
        $readmemh("negative.mem", mem);
        $display("m-2-1=%h m-2-2=%h m-1-1=%h m-1-2=%h",
                 mem[1][-2], mem[1][-1], mem[2][-2], mem[2][-1]);
    end
endmodule
