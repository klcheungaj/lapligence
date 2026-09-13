// llg-test-fixture: tests/fixtures/sim/classes/forward_const.sv
// IEEE 1800-2009 §§8.4, 8.26: forward class typedef and const property.
typedef class Foo;

class Foo;
    const int value;

    function new(int initial_value);
        value = initial_value;
    endfunction
endclass

module tb;
    Foo foo;

    initial begin
        foo = new(6);
        $display("value=%0d", foo.value);
        $finish;
    end
endmodule
