// llg-test-fixture: tests/fixtures/sim/review_batch4/mailbox_equivalent.sv
interface bus #(parameter W = 1);
    logic [W-1:0] data;
endinterface
module tb;
    bus port();
    mailbox m = new;
    virtual bus first;
    virtual bus second;
    virtual bus #(2) incompatible;
    bit signed [31:0] packed_int;
    int scalar;
    int target;
    task automatic receive(ref int output_value);
        if (m.try_get(output_value) != 1) $fatal(1, "ref mailbox destination");
    endtask
    initial begin
        first = port;
        m.put(first);
        if (m.try_get(incompatible) >= 0 || m.num() != 1) $fatal(1, "vif parameter identity");
        if (m.try_get(second) != 1 || second != first) $fatal(1, "equivalent vif declarations");
        packed_int = 23;
        m.put(packed_int);
        if (m.try_get(scalar) != 1 || scalar != 23) $fatal(1, "structurally equivalent integral type");
        m.put(scalar);
        receive(target);
        if (target != 23) $fatal(1, "ref descriptor was treated as value storage");
        $display("mailbox equivalence and ref destination ok");
        $finish(0);
    end
endmodule
