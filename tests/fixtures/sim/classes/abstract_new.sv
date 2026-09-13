// llg-test-fixture: tests/fixtures/sim/classes/abstract_new.sv
// IEEE 1800-2009 §8.20: constructing an abstract class is rejected.
virtual class AbstractBase;
    pure virtual function int run();
endclass

module tb;
    AbstractBase value;

    initial begin
        value = new();
    end
endmodule
