// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_31/compound_index_once.sv
// IEEE 1800-2009 11.4.1/11.4.2: an expression-valued compound assignment
// evaluates its target once, including a side-effecting selector, and its RHS
// once. Statement-position compound assignment to a select is tracked
// separately; this fixture pins the expression-valued sequencing contract.
module tb;
    logic [7:0] mem [0:3];
    integer i;
    integer calls;
    integer old;

    function automatic integer f;
        begin
            calls = calls + 1;
            f = 3;
        end
    endfunction

    initial begin
        mem[2] = 8'd10;
        i = 2;
        calls = 0;
        old = (mem[i++] += f());
        $display("compound mem=%0d i=%0d calls=%0d old=%0d", mem[2], i, calls, old);
        $finish(0);
    end
endmodule
