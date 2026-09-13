// llg-test-fixture: tests/fixtures/sim/partial_features/reference_array.sv
// LRM: IEEE 1800-2009 23.2.2.2 and 23.3.3
module array_leaf(ref logic [3:0] value [0:1]);
    initial begin
        #1 value[1] = 4'hc;
        $display("array child %h", value[1]);
    end
endmodule

module array_middle(ref logic [3:0] value [0:1]);
    array_leaf l(value);
endmodule

module tb;
    logic [3:0] value [0:1];
    array_middle m(value);
    initial begin
        value[0] = 4'h1;
        value[1] = 4'h2;
        #2 $display("array parent %h %h", value[0], value[1]);
        $finish(0);
    end
endmodule
