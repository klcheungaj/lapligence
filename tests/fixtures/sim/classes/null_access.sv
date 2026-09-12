// llg-test-fixture: tests/fixtures/sim/classes/null_access.sv
// IEEE 1800-2009 §8.5: dereferencing a null class handle is a runtime error.
class Box;
    int value = 9;

    function int get();
        get = value;
    endfunction
endclass

module tb;
    Box missing;

    initial begin
        missing.get();
        $finish;
    end
endmodule
