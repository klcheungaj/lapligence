// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/real_binding.sv
// IEEE 1800-2009 12.6 permits a whole-value identifier pattern; a real source
// binds its value for the filter and the true arm.
module tb;
    real value;
    initial begin
        value = 1.5;
        if (value matches .whole &&& whole > 1.0)
            $display("real binding=%0.2f twice=%0.2f", whole, whole * 2.0);
        if (value matches .low &&& low < 1.0)
            $display("unexpected %0.2f", low);
        else
            $display("filter rejected");
        $finish(0);
    end
endmodule
