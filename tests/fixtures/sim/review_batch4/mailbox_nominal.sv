// llg-test-fixture: tests/fixtures/sim/review_batch4/mailbox_nominal.sv
typedef enum int { A0 = 0, A1 = 1 } enum_a;
typedef enum int { B0 = 0, B1 = 1 } enum_b;
typedef enum_a alias_a;
class Base;
    int payload = 3;
endclass
class Derived extends Base;
endclass
class Other;
endclass
module tb;
    mailbox m = new;
    enum_a a = A1;
    alias_a same_a;
    enum_b b = B0;
    int raw = 71;
    Base base, base_copy;
    Derived derived, derived_copy;
    Other other;
    initial begin
        m.put(a);
        if (m.try_get(b) >= 0 || m.num() != 1 || b != B0) $fatal(1, "distinct enums accepted");
        if (m.try_get(raw) >= 0 || raw != 71 || m.num() != 1) $fatal(1, "enum became int");
        if (m.try_get(same_a) != 1 || same_a != A1) $fatal(1, "enum typedef alias rejected");
        derived = new;
        base = derived;
        m.put(base);
        if (m.try_get(derived_copy) >= 0 || m.num() != 1) $fatal(1, "used dynamic rather than declared class");
        if (m.try_get(base_copy) != 1 || base_copy != base) $fatal(1, "base retrieval");
        m.put(derived);
        if (m.try_get(base_copy) >= 0 || m.num() != 1) $fatal(1, "assignment compatibility is not equivalence");
        if (m.try_get(derived_copy) != 1 || derived_copy != derived) $fatal(1, "derived retrieval");
        base = null;
        m.put(base);
        if (m.try_get(other) >= 0 || m.num() != 1) $fatal(1, "null erased static type");
        if (m.try_get(base_copy) != 1 || base_copy != null) $fatal(1, "typed null retrieval");
        $display("mailbox nominal types ok");
        $finish(0);
    end
endmodule
