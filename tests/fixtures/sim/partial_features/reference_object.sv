// llg-test-fixture: tests/fixtures/sim/partial_features/reference_object.sv
// LRM: IEEE 1800-2009 23.2.2.2 and 23.3.3
module object_leaf(ref string value);
    initial begin
        #1 value = "live";
    end
endmodule

module tb;
    string value;
    object_leaf c(value);
    initial begin
        value = "";
        #2 $display("object %s", value);
        $finish(0);
    end
endmodule
