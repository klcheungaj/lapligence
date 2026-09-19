// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_21/qualified_string_inside.sv
// G1-21 case_four_state_checks (string form): unique/priority/unique0
// `case (string) inside` selects once and reports qualifier violations.
module tb;
    string s;
    logic [3:0] o;

    initial begin
        o = 4'd0;
        s = "a";
        unique case (s) inside
            "a", "b": o = 4'd1;
            "c": o = 4'd2;
            default: o = 4'd3;
        endcase
        $display("a o=%0d", o);

        s = "c";
        priority case (s) inside
            "c": o = 4'd2;
            "z": o = 4'd9;
        endcase
        $display("b o=%0d", o);

        s = "q";
        unique0 case (s) inside
            "x": o = 4'd7;
        endcase
        $display("c o=%0d", o);

        s = "n";
        unique case (s) inside
            "x": o = 4'd7;
        endcase
        $display("d o=%0d", o);
        $finish(0);
    end
endmodule
