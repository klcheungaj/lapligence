// IEEE 1364-2001 7.1.5-7.3 and IEEE 1800-2009 28.3-28.5: gate instance
// arrays distribute scalar terminals, while buf/not support multiple outputs.
module src_mod;
    reg value;
    initial begin
        value = 1'b0;
        #2 value = 1'b1;
    end
endmodule

module box_mod;
    wire [1:0] output_bus;
endmodule

module tb;
    reg [3:0] a, b;
    reg scalar;
    wire [3:0] array_y;
    wire selected_y, expression_y, mixed_y;
    wire buf_y0, buf_y1, not_y0, not_y1;
    wire hierarchy_y;
    src_mod src();
    box_mod dst();

    and array_gate[3:0](array_y, a, b);
    and selected_gate(selected_y, a[1], scalar);
    and expression_gate(expression_y, (a[0] | 1'b0), scalar);
    and mixed_gate(mixed_y, scalar, a);
    buf buffers(buf_y0, buf_y1, a[0]);
    not inverters(not_y0, not_y1, a[1]);
    and hierarchy_gate(dst.output_bus[1], src.value, scalar);
    and hierarchy_scalar(hierarchy_y, src.value, scalar);

    // More than the historical 64-terminal limit. All constants keep this
    // driver deterministic and exercise checked dynamic terminal storage.
    wire many_y;
    and many_gate(many_y,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1, 1'b1,
        1'b1);

    initial begin
        a = 4'b1010;
        b = 4'b1100;
        scalar = 1'b1;
        #1 $display("array=%b selected=%b expression=%b mixed=%b", array_y,
                   selected_y, expression_y, mixed_y);
        $display("buf=%b%b not=%b%b hierarchy=%b%b many=%b", buf_y1, buf_y0,
                 not_y1, not_y0, dst.output_bus[1], hierarchy_y, many_y);
        scalar = 1'b0;
        #1 $display("wake=%b%b%b", selected_y, expression_y, hierarchy_y);
        scalar = 1'b1;
        a[1] = 1'bx;
        #1 $display("hierwake=%b%b xz=%b%b", dst.output_bus[1], hierarchy_y,
                   not_y0, selected_y);
        $finish;
    end
endmodule
