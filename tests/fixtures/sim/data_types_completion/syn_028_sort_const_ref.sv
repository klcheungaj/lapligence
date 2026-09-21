// IEEE 1800-2009 7.12.2: one fixed-array sort fault, a const-ref receiver.
module tb;
    task automatic sort_const_ref(const ref logic [7:0] values [0:1]);
        values.sort();
    endtask

    logic [7:0] values [0:1];

    initial begin
        sort_const_ref(values);
    end
endmodule
