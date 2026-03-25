using System;
using System.Runtime.InteropServices;

namespace OxVba.TestEventServer
{
    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000002")]
    [InterfaceType(ComInterfaceType.InterfaceIsIDispatch)]
    public interface IOxVbaTestEventServerEvents
    {
        [DispId(1)]
        void OnSimpleEvent();

        [DispId(2)]
        void OnValueChanged(int value);

        [DispId(3)]
        void OnPairChanged(int a, int b);
    }

    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000003")]
    public interface IOxVbaTestEventServer
    {
        [DispId(101)]
        void FireSimpleEvent();

        [DispId(102)]
        void FireValueChanged(int value);

        [DispId(103)]
        void FirePairChanged(int a);

        [DispId(104)]
        int Ping();

        [DispId(105)]
        int ProbeSum(int lhs, int rhs);

        [DispId(106)]
        string DescribeArrayShape(object value);

        [DispId(107)]
        bool IsSelf(object value);

        [DispId(108)]
        object ProbeReturnLongArray();

        [DispId(109)]
        object ReturnSelfArray();
    }

    [ComVisible(true)]
    [Guid("E2A30001-0001-0001-0001-000000000004")]
    [ClassInterface(ClassInterfaceType.None)]
    [ComSourceInterfaces(typeof(IOxVbaTestEventServerEvents))]
    [ProgId("OxVba.TestEventServer")]
    public class TestEventServer : IOxVbaTestEventServer
    {
        public delegate void OnSimpleEventHandler();
        public delegate void OnValueChangedHandler(int value);
        public delegate void OnPairChangedHandler(int a, int b);

        public event OnSimpleEventHandler OnSimpleEvent;
        public event OnValueChangedHandler OnValueChanged;
        public event OnPairChangedHandler OnPairChanged;

        public void FireSimpleEvent()
        {
            OnSimpleEvent?.Invoke();
        }

        public void FireValueChanged(int value)
        {
            OnValueChanged?.Invoke(value);
        }

        public void FirePairChanged(int a)
        {
            OnPairChanged?.Invoke(a, a + 1);
        }

        public int Ping()
        {
            return 42;
        }

        public int ProbeSum(int lhs, int rhs)
        {
            return lhs + rhs;
        }

        public string DescribeArrayShape(object value)
        {
            if (!(value is Array array))
            {
                return "not-array";
            }

            int lowerBound = array.GetLowerBound(0);
            int upperBound = array.GetUpperBound(0);
            object first = array.Length == 0 ? null : array.GetValue(lowerBound);
            return string.Format(
                "rank={0};len={1};lb={2};ub={3};first={4}",
                array.Rank,
                array.Length,
                lowerBound,
                upperBound,
                FormatValue(first)
            );
        }

        public bool IsSelf(object value)
        {
            return ReferenceEquals(this, value);
        }

        public object ProbeReturnLongArray()
        {
            return new object[] { 4, 5, 6 };
        }

        public object ReturnSelfArray()
        {
            return new object[] { this };
        }

        private static string FormatValue(object value)
        {
            if (value == null)
            {
                return "<null>";
            }

            if (value is TestEventServer)
            {
                return "<self>";
            }

            return Convert.ToString(value, System.Globalization.CultureInfo.InvariantCulture);
        }
    }
}
