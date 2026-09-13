// llg-test-fixture: tests/fixtures/sim/classes/pure_virtual.sv
// IEEE 1800-2009 §§8.19–8.25: pure virtual contracts, protected access,
// out-of-block method definitions, and nominal dynamic dispatch.
virtual class AbstractBase;
    protected int seed;
    pure virtual function int compute(input int delta);

    function new(int initial_seed = 5);
        seed = initial_seed;
    endfunction
endclass

class Concrete extends AbstractBase;
    function new(int initial_seed = 9);
        super.new(initial_seed);
    endfunction
    extern virtual function int compute(input int delta);
endclass

function int Concrete::compute(input int delta);
    compute = seed + delta;
endfunction

module tb;
    AbstractBase base;
    Concrete concrete;

    initial begin
        concrete = new(9);
        base = concrete;
        $display("direct=%0d dynamic=%0d", concrete.compute(2), base.compute(4));
        $finish;
    end
endmodule
