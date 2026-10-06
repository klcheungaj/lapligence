// SIM-011 A03 nearest illegal form: an abstract (virtual) class cannot be
// constructed (SV 8.21).
virtual class A;
    pure virtual function int f();
endclass

module tb;
    A a;

    initial begin
        a = new;
    end
endmodule
