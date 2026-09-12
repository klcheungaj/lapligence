// llg-test-fixture: tests/fixtures/sim/unique_priority/no_match.sv
module tb;
    logic [1:0] sel;
    logic flag;
    initial begin
        sel = 2'd2;
        unique case (sel)
            2'd1: $display("unique-case");
        endcase
        unique0 case (sel)
            2'd1: $display("unique0-case");
        endcase
        priority case (sel)
            2'd1: $display("priority-case");
        endcase
        unique case (sel)
            2'd1: $display("unique-default-item");
            default: $display("case-default");
        endcase
        flag = 1'bx;
        unique if (flag)
            $display("unique-if");
        unique0 if (flag)
            $display("unique0-if");
        priority if (flag)
            $display("priority-if");
        unique if (flag)
            $display("unique-if-else");
        else
            $display("if-else");
        $finish;
    end
endmodule
