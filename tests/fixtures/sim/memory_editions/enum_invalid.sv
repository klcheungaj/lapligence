// llg-test-fixture: tests/fixtures/sim/memory_editions/enum_invalid.sv
module tb;
    typedef enum logic [1:0] {A = 2'b00, B = 2'b01} e_t;
    e_t mem [0:1];
    initial begin
        mem[0] = A;
        mem[1] = B;
        $readmemh("enum.mem", mem);
        $display("m0=%b m1=%b", mem[0], mem[1]);
    end
endmodule
