module tb;
    typedef struct packed {
        logic [7:0] four;
        bit   [7:0] two;
    } mix_t;

    mix_t v;
    initial begin
        v = '0;
        v.four = 8'hx;
        v.two  = 8'hx;
        if (v.four !== 8'hxx) begin
            $display("FAIL four=%h", v.four);
            $finish;
        end
        if (v.two !== 8'h00) begin
            $display("FAIL two=%h", v.two);
            $finish;
        end
        $display("PASS packed_member_state_boundary");
        $finish;
    end
endmodule
