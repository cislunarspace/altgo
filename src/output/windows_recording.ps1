try {
    Add-Type -AssemblyName PresentationFramework
    Add-Type -AssemblyName PresentationCore
    Add-Type -AssemblyName WindowsBase

    $xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        WindowStyle="None" AllowsTransparency="True"
        Background="#CC1A1A1A" Opacity="0.95"
        ShowInTaskbar="False" Topmost="True"
        SizeToContent="Height" Width="320"
        WindowStartupLocation="Manual"
        Left="{{left}}" Top="{{top}}">
    <Border CornerRadius="16" Padding="24,20" Background="#CC1A1A1A"
            BorderBrush="#33FFFFFF" BorderThickness="1">
        <StackPanel HorizontalAlignment="Center">
            <TextBlock Text="🎙️" FontSize="36" HorizontalAlignment="Center" Margin="0,0,0,8"/>
            <TextBlock Text="正在说话..." Foreground="#CCCCCC"
                       FontSize="15" FontWeight="SemiBold" HorizontalAlignment="Center"/>
        </StackPanel>
    </Border>
</Window>
"@

    $reader = [System.Xml.XmlReader]::Create([System.IO.StringReader]::new($xaml))
    $window = [System.Windows.Markup.XamlReader]::Load($reader)
    $window.ShowDialog() | Out-Null
} catch {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show("Recording...", "altgo", "OK", "Information")
}
