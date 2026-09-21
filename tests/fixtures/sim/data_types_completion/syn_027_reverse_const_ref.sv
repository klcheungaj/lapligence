// IEEE 1800-2009 7.12.2: reverse cannot write through a const ref receiver.
module tb;
    task automatic reverse_const_ref(const ref logic [7:0] values [0:1]);
        values.reverse();
    endtask

    logic [7:0] values [0:1];

    initial begin
        reverse_const_ref(values);
    end
endmodule
