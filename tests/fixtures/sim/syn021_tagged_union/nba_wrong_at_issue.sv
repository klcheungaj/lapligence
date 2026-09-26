// IEEE 1800-2009 §§4.9.4 and 11.9: the left target uses issue-time values.
module tb;
    typedef union tagged packed { logic [7:0] A; logic [7:0] B; } item_t;
    item_t value;
    initial begin
        value = tagged B(8'h55);
        value.A <= 8'h33;
        value = tagged A(8'h11);
        #1;
        if (value.A !== 8'h11)
            $fatal(1, "an invalid issue-time target wrote after retagging");
        $display("AFTER_WRONG_ISSUE active_A=%h", value.A);
        $finish;
    end
endmodule
