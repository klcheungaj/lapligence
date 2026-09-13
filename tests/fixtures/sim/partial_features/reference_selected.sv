// llg-test-fixture: tests/fixtures/sim/partial_features/reference_selected.sv
// LRM: IEEE 1800-2009 23.2.2.2 and 23.3.3
module selected_leaf(ref logic [3:0] a);
    initial begin
        #1 a = 4'h5;
        $display("selected child %h %h %h", a, tb.a, tb.m.a);
    end
endmodule

module selected_middle(ref logic [3:0] a);
    selected_leaf l(a);
endmodule

module tb;
    logic [7:0] a;
    selected_middle m(a[7:4]);
    initial begin
        a = 8'h00;
        #2 $display("selected parent %h %h %h", a, m.a, m.l.a);
        $finish(0);
    end
endmodule
