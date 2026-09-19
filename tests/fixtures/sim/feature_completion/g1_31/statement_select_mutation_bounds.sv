// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_31/statement_select_mutation_bounds.sv
// IEEE 1800-2009 7.4.6 and 11.4.1: an out-of-range or unknown array index
// makes the compound store a no-op but the RHS is still evaluated exactly
// once; an in-range index commits one store.
module tb;
    logic [7:0] mem [0:1];
    integer i;
    integer calls;

    function automatic integer f;
        begin
            calls = calls + 1;
            f = 1;
        end
    endfunction

    initial begin
        mem[0] = 8'h10;
        mem[1] = 8'h20;
        calls = 0;

        i = 5;
        mem[i] += f();
        mem[i]++;
        $display("oob %h %h calls=%0d", mem[0], mem[1], calls);

        i = 32'bxxxxxxxx;
        mem[i] += f();
        mem[i]++;
        $display("unknown %h %h calls=%0d", mem[0], mem[1], calls);

        i = 1;
        mem[i] += f();
        $display("in %h %h calls=%0d", mem[0], mem[1], calls);
        $finish(0);
    end
endmodule
