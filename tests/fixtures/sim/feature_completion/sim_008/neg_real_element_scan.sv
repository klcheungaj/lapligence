// SIM-008 boundary: a real element of a container as a scan destination is
// legal (SV 21.3.4.3) but real references have no retained element cell yet.
module tb;
    real rq[$] = '{0.0};
    initial void'($sscanf("1.5", "%f", rq[0]));
endmodule
