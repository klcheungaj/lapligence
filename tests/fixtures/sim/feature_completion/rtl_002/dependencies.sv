// IEEE 1800-2009 §§9.4, 10.4: stable selected cells and whole-array notification.
module tb;
    logic [7:0] source [0:16777215], target [0:16777215];
    bit selected_woke, dynamic_woke;
    logic all_same;
    always_comb all_same = (source === target);
    integer index = 2;
    initial begin
        source[2] = 8'h12;
        #1;
        if (all_same !== 1'b0) $fatal;
        target = source;
        #1;
        if (!selected_woke || !dynamic_woke || all_same !== 1'b1) $fatal;
        $display("PASS rtl002 dependencies");
        $finish(0);
    end
    initial begin
        @(target[2]);
        if (target[2] !== 8'h12) $fatal;
        selected_woke = 1;
    end
    initial begin
        @(target[index]);
        if (target[index] !== 8'h12) $fatal;
        dynamic_woke = 1;
    end
endmodule
