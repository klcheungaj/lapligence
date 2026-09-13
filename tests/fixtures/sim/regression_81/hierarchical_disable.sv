// llg-test-fixture: tests/fixtures/sim/regression_81/hierarchical_disable.sv
// IEEE 1800-2009 section 9.6.2: a hierarchical disable resolves the selected
// elaborated instance rather than aliasing equal source names in a sibling.
module child;
    integer hit = 0;
    initial begin : target
        #5 hit = 1;
    end
endmodule

module tb;
    child u0();
    child u1();

    initial begin
        #1 disable u0.target;
        #6 $display("u0=%0d u1=%0d", u0.hit, u1.hit);
        $finish(0);
    end
endmodule
