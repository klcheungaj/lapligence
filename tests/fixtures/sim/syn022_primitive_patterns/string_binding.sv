// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/string_binding.sv
// IEEE 1800-2009 12.6 permits a whole-value identifier pattern; a string
// source binds its value for the filter and the true arm.
module tb;
    string value;
    initial begin
        value = "abc";
        if (value matches .whole &&& whole.len() == 3)
            $display("native binding=%s joined=%s", whole, {whole, "!"});
        if (value matches .other &&& other == "xyz")
            $display("unexpected %s", other);
        else
            $display("filter rejected");
        $finish(0);
    end
endmodule
